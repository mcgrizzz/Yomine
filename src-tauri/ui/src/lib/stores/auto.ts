import { derived, get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { autoPick } from '$lib/autopick';
import { applyControls } from '$lib/table';
import { autoLedger, mineQueue } from './batches';
import { freqFilter, jlptEnabled, posEnabled } from './controls';
import { fileResult } from './file';
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
	yomitanReachable
} from './mining';
import { playerStatus } from './player';
import { ankiStatus } from './status';
import { settings } from './settings';
import { lastError, showNotice } from './ui';

export const autoMode = writable(false);

export const autoAvailable = derived(
	[yomitanReachable, ankiStatus, playerStatus],
	([$yomitan, $anki, $player]) => $yomitan && $anki.connected && $player.ws_clients > 0
);

export const autoCountdown = writable<number | null>(null);

/** Fingerprint of the loaded video auto mode skipped as already mined. */
export const autoSkipped = writable<string | null>(null);

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
			return;
		}
	} catch (err) {
		lastError.set({ title: 'Auto mode', message: String(err), detail: null });
		return;
	}
	if (!get(autoMode) || get(fileResult)?.batch_source.fingerprint !== fingerprint) return;
	cancelAutoCountdown();
	let secs = 3;
	autoCountdown.set(secs);
	countdownTimer = setInterval(() => {
		if (!get(autoMode) || get(fileResult)?.batch_source.fingerprint !== fingerprint) {
			cancelAutoCountdown();
		} else if (secs > 1) {
			autoCountdown.set(--secs);
		} else {
			cancelAutoCountdown();
			void autoMine();
		}
	}, 1000);
}

export async function setAutoMode(on: boolean): Promise<void> {
	autoMode.set(on);
	autoSkipped.set(null);
	if (!on) cancelAutoCountdown();
	try {
		await ipc.setAutoMode(on);
	} catch (err) {
		autoMode.set(false);
		lastError.set({ title: 'Auto mode', message: String(err), detail: null });
		return;
	}
	if (on) void countDownCurrent();
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
		isMined: (t) => isMinedTerm(t, mined, added),
		minedSentences: new Set([...get(minedSentences), ...get(sessionMinedSentences)]),
		normalize: normalizeSentence
	});
}

/** Mines the loaded asbplayer video once, unless it was already processed and not `force`d. */
export async function autoMine(force = false): Promise<void> {
	if (picking) {
		loadedWhilePicking = true;
		return;
	}
	const loaded = get(fileResult);
	const prefs = get(settings)?.auto_mine;
	if (!get(autoMode) || !loaded || !prefs) return;
	const { fingerprint } = loaded.batch_source;
	const title = loaded.source_file.title;
	if (get(playerBusy) || get(miningTerm) !== null) {
		showNotice(`Auto mode skipped ${title}: a mine was already running`);
		return;
	}
	picking = true;
	autoSkipped.set(null);
	try {
		if (!force && (await ipc.isMediaProcessed(fingerprint))) {
			autoSkipped.set(fingerprint);
			return;
		}
		await refreshMinedState(true);
		const file = get(fileResult);
		if (!get(autoMode) || file?.batch_source.fingerprint !== fingerprint) return;
		const items = pick(file, prefs);
		if (items.length === 0) {
			await ipc.markMediaProcessed(fingerprint);
			autoLedger.update((l) => [...l, { title, batchId: null, created: 0, undone: false }]);
			showNotice(`Auto mode found nothing to mine in ${title}`);
			return;
		}
		const batch = await mineQueue(items, true);
		if (!batch) return;
		await ipc.markMediaProcessed(fingerprint);
		const created = batch.items.filter((i) => i.outcome.status === 'created').length;
		autoLedger.update((l) => [...l, { title, batchId: batch.id, created, undone: false }]);
	} catch (err) {
		lastError.set({ title: 'Auto mode', message: String(err), detail: null });
	} finally {
		picking = false;
		if (loadedWhilePicking) {
			loadedWhilePicking = false;
			void autoMine();
		}
	}
}
