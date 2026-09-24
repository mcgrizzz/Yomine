import { get, writable, type Writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import {
	cueSecs,
	estimateProgress,
	lacksMedia,
	needsReview,
	occurrenceIndex,
	retryIndices,
	sameSource,
	type BatchPhase,
	type BatchPlan
} from '$lib/batch';
import { fileResult } from './file';
import { asbContext, playerStatus } from './player';
import { adhocQueue, dropAdhoc, queuedMineOptions, queueAdhoc, setSelected } from './selection';
import {
	mediaMissing,
	minedNoteIds,
	minedTerms,
	miningTerm,
	normalizeSentence,
	playerBusy,
	refreshMinedState,
	sessionMinedSentences,
	type QueueItem
} from './mining';
import { batchSummaryOpen } from './modals';
import { lastError, showNotice } from './ui';

export type PauseChoice = 'retry' | 'without_dictionary_media' | 'skip' | 'stop';

export interface BatchPauseState {
	item: ipc.BatchItem;
	failure: ipc.BatchFailure;
}

export interface BatchProgress {
	phase: BatchPhase;
	position: number;
	count: number;
	recordsNext: boolean;
	doneSecs: number;
	currentSecs: number;
	totalSecs: number;
	estimatedAt: number;
	current: string;
	sentence: string;
	key: string;
	message: string | null;
}

export interface BatchTargetChoice {
	media: ipc.BoundMedia[];
	mediaRun: boolean;
}

type TargetAnswer = { target: string | null; record: boolean } | null;

function prompt<T, R>(cancelValue: R) {
	const shown: Writable<T | null> = writable(null);
	let resolver: ((answer: R) => void) | null = null;
	return {
		shown,
		ask: (value: T) =>
			new Promise<R>((resolve) => {
				resolver = resolve;
				shown.set(value);
			}).finally(() => {
				resolver = null;
				shown.set(null);
			}),
		answer: (answer: R) => resolver?.(answer),
		cancel: () => resolver?.(cancelValue)
	};
}

const pausePrompt = prompt<BatchPauseState, PauseChoice>('stop');
const targetPrompt = prompt<BatchTargetChoice, TargetAnswer>(null);
const replacePrompt = prompt<ipc.BatchRecord, boolean>(false);

export const batchPause = pausePrompt.shown;
export const batchTarget = targetPrompt.shown;
export const batchReplace = replacePrompt.shown;
export const batchSaveError = writable<string | null>(null);
export const lastBatch = writable<ipc.BatchRecord | null>(null);
export const mineQueueState = writable<BatchProgress | null>(null);
export interface BatchPreview {
	lemma: string;
	sentence: string;
	src: string;
}

export const batchPreview = writable<BatchPreview | null>(null);

let cancelled = false;

export function pauseChoices(pause: BatchPauseState): PauseChoice[] {
	const { fallback, scope } = pause.failure;
	const choices: PauseChoice[] = ['retry'];
	if (fallback === 'without_dictionary_media') choices.push('without_dictionary_media');
	if (scope !== 'shared') choices.push('skip');
	return [...choices, 'stop'];
}

export function resumeBatch(choice: PauseChoice): void {
	const pause = get(batchPause);
	if (pause && pauseChoices(pause).includes(choice)) pausePrompt.answer(choice);
}

export const selectBatchTarget = (answer: TargetAnswer): void => targetPrompt.answer(answer);
export const confirmReplaceBatch = (replace: boolean): void => replacePrompt.answer(replace);

export function cancelQueue(): void {
	cancelled = true;
	pausePrompt.cancel();
	targetPrompt.cancel();
	replacePrompt.cancel();
}

export async function loadLastBatch(): Promise<void> {
	const current = get(lastBatch);
	try {
		const saved = await ipc.getLastBatch();
		if (get(lastBatch) === current && !get(playerBusy)) lastBatch.set(saved);
	} catch (error) {
		report(error);
	}
}

function report(error: unknown): void {
	lastError.set({ title: 'Batch mining', message: String(error), detail: null });
}

async function chooseTarget(mediaRun: boolean): Promise<TargetAnswer> {
	const media = await ipc.getAsbplayerMedia().catch(() => []);
	if (cancelled) return null;
	const usable = media.filter((m) => m.active && m.loaded_subtitles.length > 0);
	if (usable.length === 1) return { target: usable[0].id, record: true };
	return targetPrompt.ask({ media, mediaRun });
}

async function showPreview(item: ipc.BatchItem, file: string): Promise<void> {
	const src = await ipc.getMediaPreview(file).catch(() => null);
	if (src && get(playerBusy)) batchPreview.set({ lemma: item.lemma, sentence: item.sentence, src });
}

function recordSuccess(item: ipc.BatchItem): void {
	const outcome = item.outcome;
	if (outcome.status !== 'created' && outcome.status !== 'duplicate') return;
	minedTerms.update((s) => new Set(s).add(item.lemma));
	if (outcome.status !== 'created') return;
	if (item.sentence) {
		sessionMinedSentences.update((s) => new Set(s).add(normalizeSentence(item.sentence)));
	}
	minedNoteIds.update((m) => ({ ...m, [item.lemma]: outcome.note_id }));
	mediaMissing.update((s) => {
		const next = new Set(s);
		if (lacksMedia(outcome)) next.add(item.lemma);
		else next.delete(item.lemma);
		return next;
	});
}

function clearUnchangedSelection(item: ipc.BatchItem): void {
	const status = item.outcome.status;
	if (status !== 'created' && status !== 'duplicate') return;
	if (item.adhoc) {
		const queued = get(adhocQueue).find((a) => a.key === item.key);
		const unchanged =
			queued &&
			queued.sentence === item.sentence &&
			(queued.entryIndex ?? null) === item.entry_index &&
			(queued.formatName ?? null) === item.format_name &&
			(queued.scanText ?? null) === item.scan_text;
		if (unchanged) dropAdhoc(item.key);
		return;
	}
	const file = get(fileResult);
	if (!file) return;
	const option = get(queuedMineOptions)[item.key];
	const index = occurrenceIndex(item, file);
	const unchanged =
		index !== null &&
		(option?.occIdx ?? 0) === index &&
		(option?.entryIndex ?? null) === item.entry_index &&
		(option?.formatName ?? null) === item.format_name &&
		(option?.scanText ?? null) === item.scan_text;
	if (unchanged) setSelected([item.key], false);
}

function toBatchItems(items: QueueItem[]): ipc.BatchItem[] {
	const status = get(playerStatus);
	const recording = status.mode === 'asbplayer' || get(asbContext).loaded_from_asbplayer;
	const adhoc = new Set(get(adhocQueue).map((a) => a.key));
	const start = (i: QueueItem) => i.timestamp?.start_secs ?? Infinity;
	return [...items]
		.sort((a, b) => start(a) - start(b))
		.map((i) => ({
			key: i.key,
			lemma: i.lemma,
			surface: i.surface,
			sentence: i.sentence,
			timestamp: i.timestamp,
			entry_index: i.entryIndex ?? null,
			format_name: i.formatName ?? null,
			scan_text: i.scanText ?? null,
			adhoc: adhoc.has(i.key),
			mine_media: i.timestamp !== null && recording,
			outcome: { status: 'unattempted' }
		}));
}

async function run(
	items: QueueItem[] | null,
	retry: { indices: number[]; media: boolean } | null = null,
	auto = false
): Promise<ipc.BatchRecord | null> {
	if (get(playerBusy) || get(miningTerm) !== null) return null;
	const file = get(fileResult);
	if (!file) return null;
	const previous = get(lastBatch);
	if (!items && (!previous || !sameSource(previous, file))) {
		report('Load the original source before retrying this batch');
		return null;
	}
	if (items?.length === 0 || retry?.indices.length === 0) return null;
	const mediaRun = retry?.media ?? false;
	playerBusy.set(true);
	cancelled = false;
	batchPreview.set(null);
	batchSummaryOpen.set(false);
	let batch = items ? null : previous;
	let fatal = false;
	let started = false;
	const options: ipc.MineOptions = { record: true, require_dictionary_media: true };
	let target: string | null = null;
	let finished: ipc.BatchRecord | null = null;

	const plan: BatchPlan = {
		create: { indices: [], done: 0, samples: [] },
		record: { indices: [], done: 0, samples: [] }
	};

	function publish(phase: BatchPhase, item: ipc.BatchItem): void {
		if (!batch) return;
		mineQueueState.set({
			phase,
			position: plan[phase].done + 1,
			count: plan[phase].indices.length,
			recordsNext: phase === 'create' && plan.record.indices.length > 0,
			...estimateProgress(batch, plan),
			estimatedAt: Date.now(),
			current: item.lemma,
			sentence: item.sentence,
			key: item.key,
			message: null
		});
	}

	async function runPhase(phase: BatchPhase): Promise<void> {
		for (const index of plan[phase].indices) {
			if (cancelled || fatal || !batch) return;
			const item = batch.items[index];
			miningTerm.set(item.lemma);
			options.require_dictionary_media = true;
			for (;;) {
				publish(phase, item);
				const startedAt = performance.now();
				const step = await ipc.mineBatchItem(batch.id, index, target, { ...options }, (m) => {
					if (m.message) mineQueueState.update((s) => s && { ...s, message: m.message });
				});
				const secs = (performance.now() - startedAt) / 1000;
				batch = step.batch;
				lastBatch.set(batch);
				const current = batch.items[index];
				recordSuccess(current);
				if (step.preview_file) void showPreview(current, step.preview_file);
				if (step.failure?.scope === 'stop') {
					fatal = true;
					if (step.failure.stage === 'Saving batch') batchSaveError.set(step.failure.message);
					report(step.failure.message);
					return;
				}
				if (!step.failure) {
					plan[phase].samples.push(phase === 'record' ? Math.max(0, secs - cueSecs(current)) : secs);
				}
				clearUnchangedSelection(current);
				if (!step.failure || cancelled) break;
				const choice = await pausePrompt.ask({ item: current, failure: step.failure });
				if (choice === 'retry') continue;
				if (choice === 'without_dictionary_media') {
					options.require_dictionary_media = false;
					continue;
				}
				if (choice === 'stop') cancelled = true;
				break;
			}
			plan[phase].done++;
		}
	}

	try {
		await ipc.setBatchRunning(true);
		if (items && previous && needsReview(previous) && !(await replacePrompt.ask(previous))) {
			if (!cancelled) batchSummaryOpen.set(true);
			return null;
		}
		const snapshot = items ? toBatchItems(items) : previous!.items;
		const selected = retry?.indices ?? snapshot.map((_, i) => i);
		const records = selected.some((i) => snapshot[i].mine_media);
		if (records && !get(asbContext).loaded_from_asbplayer) {
			const answer = await chooseTarget(mediaRun);
			if (!answer || cancelled) return null;
			target = answer.target;
			options.record = answer.record;
		}
		if (items) {
			batch = await ipc.createBatch(file.batch_source, snapshot);
			batchSaveError.set(null);
		}
		if (!batch) return null;
		lastBatch.set(batch);
		started = true;
		if (mediaRun) {
			plan.record.indices = selected;
		} else {
			plan.create.indices = selected;
			if (options.record) plan.record.indices = selected.filter((i) => snapshot[i].mine_media);
			await runPhase('create');
			const created = batch.items;
			plan.record.indices = plan.record.indices.filter((i) => {
				const o = created[i].outcome;
				return o.status === 'created' && o.media === 'pending';
			});
		}
		await runPhase('record');
	} catch (error) {
		fatal = true;
		report(error);
		if (batch && started) {
			try {
				const saved = await ipc.getLastBatch();
				if (saved?.id === batch.id) {
					batch = saved;
					lastBatch.set(saved);
				}
			} catch (readError) {
				batchSaveError.set(String(readError));
			}
		}
	} finally {
		if (batch && started && !fatal) {
			try {
				batch = await ipc.finishBatch(batch.id);
				lastBatch.set(batch);
				if (!cancelled) finished = batch;
			} catch (error) {
				batchSaveError.set(String(error));
				report(error);
			}
		}
		void ipc.setBatchRunning(false);
		miningTerm.set(null);
		playerBusy.set(false);
		mineQueueState.set(null);
		batchPreview.set(null);
		if (batch) {
			if (!auto || !finished) batchSummaryOpen.set(true);
			void refreshMinedState(true);
		}
	}
	return finished;
}

export const mineQueue = (items: QueueItem[], auto = false): Promise<ipc.BatchRecord | null> =>
	run(items, null, auto);

export function retryBatch(media = false): Promise<ipc.BatchRecord | null> {
	const batch = get(lastBatch);
	if (get(batchSaveError) || !batch) return Promise.resolve(null);
	return run(null, { indices: retryIndices(batch, media), media });
}

export function restoreBatchSelection(): void {
	if (get(playerBusy)) return;
	const batch = get(lastBatch);
	const file = get(fileResult);
	if (!batch || !file || !sameSource(batch, file)) {
		report('Load the original source to restore this selection');
		return;
	}
	let restored = 0;
	for (const item of batch.items) {
		const occIdx = occurrenceIndex(item, file);
		if (occIdx === null) continue;
		const options = {
			entryIndex: item.entry_index ?? undefined,
			formatName: item.format_name ?? undefined,
			scanText: item.scan_text ?? undefined
		};
		if (item.adhoc) {
			queueAdhoc({
				key: item.key,
				lemma: item.lemma,
				surface: item.surface,
				sentence: item.sentence,
				timestamp: item.timestamp,
				...options
			});
		} else {
			queuedMineOptions.update((m) => ({ ...m, [item.key]: { ...options, occIdx, userChosen: true } }));
			setSelected([item.key], true);
		}
		restored++;
	}
	const unavailable = restored < batch.items.length ? ' — some original occurrences are unavailable' : '';
	showNotice(`Restored ${restored} of ${batch.items.length} items${unavailable}`);
}

export async function undoLastBatch(): Promise<void> {
	if (get(batchSaveError) || get(playerBusy) || get(miningTerm) !== null) return;
	const batch = get(lastBatch);
	if (!batch) return;
	playerBusy.set(true);
	try {
		const result = await ipc.undoBatch(batch.id);
		lastBatch.set(result.batch);
		const deleted = new Set(
			result.batch.items.flatMap((i) => (i.outcome.status === 'deleted' ? [i.outcome.note_id] : []))
		);
		const ids = get(minedNoteIds);
		const lemmas = new Set(Object.keys(ids).filter((lemma) => deleted.has(ids[lemma])));
		minedNoteIds.set(Object.fromEntries(Object.entries(ids).filter(([, id]) => !deleted.has(id))));
		mediaMissing.update((s) => new Set([...s].filter((lemma) => !lemmas.has(lemma))));
		await refreshMinedState(true);
		const remaining = result.remaining ? ` · ${result.remaining} could not be deleted` : '';
		showNotice(`Deleted ${result.deleted} notes · ${result.already_gone} already gone${remaining}`);
	} catch (error) {
		report(error);
	} finally {
		playerBusy.set(false);
	}
}
