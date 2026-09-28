import { derived, get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { autoPick, DEFAULT_HORIZON, type NextInLine } from '$lib/autopick';
import { applyControls } from '$lib/table';
import { autoLedger, mineQueue } from './batches';
import { freqFilter, jlptEnabled, posEnabled } from './controls';
import { advanceQueue, fileResult, localVideo } from './file';
import { showPossibleKnownMatches } from './knowledgeView';
import {
	addedTerms,
	isMinedTerm,
	minedSentences,
	minedTerms,
	miningTerm,
	normalizeSentence,
	playerBusy,
	refreshMinedState,
	sessionMinedSentences,
	yomitanReachable,
	type QueueItem
} from './mining';
import { batchSummaryOpen, reviewDialogOpen } from './modals';
import { playerStatus } from './player';
import { setSelected } from './selection';
import { ankiStatus, knowledge } from './status';
import { miningMode, setAutoReview, settings } from './settings';
import { lastError, showNotice } from './ui';

/** What happens when a new video loads: nothing, a review of the picks, or mining them. */
export type AutoMode = 'off' | 'review' | 'mine';

export const autoMode = writable<AutoMode>('off');

export const autoAvailable = derived(
	[yomitanReachable, ankiStatus, playerStatus, miningMode, localVideo],
	([$yomitan, $anki, $player, $mode, $video]) =>
		$yomitan &&
		$anki.connected &&
		($mode === 'local' ? $video !== null : $player.ws_clients > 0)
);

export const autoCountdown = writable<number | null>(null);

/** Fingerprint of the loaded video auto mode skipped as already mined. */
export const autoSkipped = writable<string | null>(null);

/** Picks waiting for review, selected in the table until mined or skipped. */
export interface AutoReview {
	fingerprint: string;
	title: string;
	keys: string[];
	next: NextInLine[];
}

const review = writable<AutoReview | null>(null);

/** The review for the loaded video, if one is waiting. */
export const autoReview = derived([review, fileResult], ([$review, $file]) =>
	$review && $review.fingerprint === $file?.batch_source.fingerprint ? $review : null
);

let picking = false;
let loadedWhilePicking = false;
let countdownTimer: ReturnType<typeof setInterval> | null = null;

function cancelAutoCountdown(): void {
	if (!countdownTimer) return;
	clearInterval(countdownTimer);
	countdownTimer = null;
	autoCountdown.set(null);
}

async function countDownCurrent(): Promise<void> {
	const file = get(fileResult);
	if (!file?.sentences.some((s) => s.timestamp)) return;
	const { fingerprint } = file.batch_source;
	try {
		if (await ipc.isMediaProcessed(fingerprint)) {
			autoSkipped.set(fingerprint);
			void advanceQueue(fingerprint, true);
			return;
		}
	} catch (err) {
		lastError.set({ title: 'Auto mode', message: String(err), detail: null });
		return;
	}
	if (get(autoMode) !== 'mine' || get(fileResult)?.batch_source.fingerprint !== fingerprint) return;
	cancelAutoCountdown();
	let secs = 3;
	autoCountdown.set(secs);
	countdownTimer = setInterval(() => {
		if (get(autoMode) !== 'mine' || get(fileResult)?.batch_source.fingerprint !== fingerprint) {
			cancelAutoCountdown();
		} else if (secs > 1) {
			autoCountdown.set(--secs);
		} else {
			cancelAutoCountdown();
			void autoMine();
		}
	}, 1000);
}

export async function setAutoMode(mode: AutoMode): Promise<void> {
	const previous = get(autoMode);
	if (mode === previous) return;
	autoMode.set(mode);
	autoSkipped.set(null);
	cancelAutoCountdown();
	review.set(null);
	reviewDialogOpen.set(false);
	try {
		await ipc.setAutoMode(mode !== 'off');
	} catch (err) {
		autoMode.set(previous);
		lastError.set({ title: 'Auto mode', message: String(err), detail: null });
		return;
	}
	void setAutoReview(mode !== 'off');
	if (mode === 'mine') void countDownCurrent();
	if (mode === 'review') void reviewVideo();
}

export function onNewVideo(): void {
	const mode = get(autoMode);
	if (mode === 'mine') void countDownCurrent();
	if (mode === 'review') void reviewVideo();
}

/** In asbplayer mode, asbplayer's loads count as new videos instead. */
export function onLocalVideo(): void {
	if (get(miningMode) === 'local') onNewVideo();
}

/** Runs the video again although it was already processed ("Mine anyway"). */
export function processAnyway(): Promise<void> {
	return get(autoMode) === 'review' ? reviewVideo(true) : autoMine(true);
}

function pick(file: ipc.FileLoadResult, prefs: ipc.AutoMine) {
	const terms = applyControls(file.terms, file.sentences, {
		search: '',
		showPossibleKnownMatches: get(showPossibleKnownMatches),
		sort: { field: 'frequency', dir: 'asc' },
		pos: get(posEnabled),
		freq: get(freqFilter),
		jlpt: get(jlptEnabled)
	});
	const mined = get(minedTerms);
	const added = get(addedTerms);
	return autoPick(terms, file.sentences, {
		prefs,
		horizon: get(knowledge)?.horizon ?? DEFAULT_HORIZON,
		isMined: (t) => isMinedTerm(t, mined, added),
		minedSentences: new Set([...get(minedSentences), ...get(sessionMinedSentences)]),
		normalize: normalizeSentence
	});
}

/** Selects auto mode's picks, each on its chosen sentence. */
export async function selectPicks(): Promise<ReturnType<typeof pick> | null> {
	const prefs = get(settings)?.auto_mine;
	if (!prefs || !get(fileResult)) return null;
	await refreshMinedState(true);
	const file = get(fileResult);
	if (!file) return null;
	const result = pick(file, prefs);
	const occIdx = new Map(result.picks.map((p) => [p.item.key, p.occIdx]));
	setSelected([...occIdx.keys()], true, (key) => ({ occIdx: occIdx.get(key) ?? 0, userChosen: false }));
	return result;
}

/** Skips `run` while a pick is running; a video loaded meanwhile is picked after. */
async function exclusive(run: () => Promise<void>): Promise<void> {
	if (picking) {
		loadedWhilePicking = true;
		return;
	}
	picking = true;
	try {
		await run();
	} catch (err) {
		lastError.set({ title: 'Auto mode', message: String(err), detail: null });
	} finally {
		picking = false;
		if (loadedWhilePicking) {
			loadedWhilePicking = false;
			onNewVideo();
		}
	}
}

/** Selects the loaded video's picks and opens the review dialog, unless it was processed. */
function reviewVideo(force = false): Promise<void> {
	return exclusive(async () => {
		const loaded = await claimVideo('review', force);
		if (!loaded) return;
		const { fingerprint } = loaded.batch_source;
		const result = await selectPicks();
		if (!result || get(autoMode) !== 'review') return;
		if (get(fileResult)?.batch_source.fingerprint !== fingerprint) return;
		const title = loaded.source_file.title;
		if (result.picks.length === 0) {
			showNotice(`Auto mode found nothing to mine in ${title}`);
			void advanceQueue(fingerprint);
			return;
		}
		review.set({ fingerprint, title, keys: result.picks.map((p) => p.item.key), next: result.next });
		reviewDialogOpen.set(true);
	});
}

/** A term added to a waiting review counts as one of its picks. */
export function addReviewPick(key: string): void {
	review.update((r) => (r && !r.keys.includes(key) ? { ...r, keys: [...r.keys, key] } : r));
}

/** Drops a waiting review once none of its picks are selected, as Skip would. */
export function dropDeselectedReview(selected: Set<string>): void {
	const pending = get(review);
	if (pending && !get(reviewDialogOpen) && !pending.keys.some((k) => selected.has(k)))
		review.set(null);
}

/** Drops the review and the picks it selected, without marking the video processed. */
export function skipReview(): void {
	const pending = get(autoReview);
	if (pending) setSelected(pending.keys, false);
	review.set(null);
	reviewDialogOpen.set(false);
	if (pending) void advanceQueue(pending.fingerprint);
}

/** The loaded video, if `mode` is on, nothing is mining, and it wasn't processed (unless
 * `force`d). */
async function claimVideo(mode: AutoMode, force: boolean): Promise<ipc.FileLoadResult | null> {
	const loaded = get(fileResult);
	if (get(autoMode) !== mode || !loaded) return null;
	if (get(playerBusy) || get(miningTerm) !== null) {
		showNotice(`Auto mode skipped ${loaded.source_file.title}: a mine was already running`);
		return null;
	}
	const { fingerprint } = loaded.batch_source;
	autoSkipped.set(null);
	if (!force && (await ipc.isMediaProcessed(fingerprint))) {
		autoSkipped.set(fingerprint);
		void advanceQueue(fingerprint, true);
		return null;
	}
	return loaded;
}

async function recordProcessed(fingerprint: string, title: string, batch: ipc.BatchRecord | null) {
	await ipc.markMediaProcessed(fingerprint);
	const created = batch?.items.filter((i) => i.outcome.status === 'created').length ?? 0;
	autoLedger.update((l) => [...l, { title, batchId: batch?.id ?? null, created, undone: false }]);
}

/** Mining a waiting review's picks marks its video processed. */
export async function mineSelection(items: QueueItem[]): Promise<void> {
	const pending = get(autoReview);
	const batch = await mineQueue(items, pending !== null);
	if (!pending || !batch) return;
	review.set(null);
	await recordProcessed(pending.fingerprint, pending.title, batch).catch((err) =>
		lastError.set({ title: 'Auto mode', message: String(err), detail: null })
	);
	// The next video's review would open over this batch's summary.
	await new Promise<void>((resolve) => {
		const stop = batchSummaryOpen.subscribe((open) => {
			if (open) return;
			queueMicrotask(() => stop());
			resolve();
		});
	});
	void advanceQueue(pending.fingerprint);
}

/** Mines the loaded video once, unless it was already processed and not `force`d. */
export function autoMine(force = false): Promise<void> {
	return exclusive(async () => {
		const prefs = get(settings)?.auto_mine;
		const loaded = prefs ? await claimVideo('mine', force) : null;
		if (!prefs || !loaded) return;
		const { fingerprint } = loaded.batch_source;
		const title = loaded.source_file.title;
		await refreshMinedState(true);
		const file = get(fileResult);
		if (get(autoMode) !== 'mine' || file?.batch_source.fingerprint !== fingerprint) return;
		const items = pick(file, prefs).picks.map((p) => p.item);
		if (items.length === 0) {
			await recordProcessed(fingerprint, title, null);
			showNotice(`Auto mode found nothing to mine in ${title}`);
			void advanceQueue(fingerprint);
			return;
		}
		const batch = await mineQueue(items, true);
		if (!batch) return;
		await recordProcessed(fingerprint, title, batch);
		void advanceQueue(fingerprint);
	});
}
