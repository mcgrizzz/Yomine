// One-click mining (issue #105), batch mining (issue #114) + already-mined
// state (issue #3).

import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { termKey } from '$lib/table';
import { playerStatus } from './player';
import { clearSelection, selectedTerms } from './selection';
import { lastError, showNotice } from './ui';

/** Lemmas mined this session (optimistic, until the next refresh). */
export const minedTerms = writable<Set<string>>(new Set());
/** Terms with an Anki card added in the last day (`added:1`). */
export const addedTerms = writable<Set<string>>(new Set());
/** Normalized sentences that already exist in the user's notes. */
export const minedSentences = writable<Set<string>>(new Set());
/** Normalized sentences mined this session (optimistic). */
export const sessionMinedSentences = writable<Set<string>>(new Set());
/** Lemma currently being mined (one mine at a time). */
export const miningTerm = writable<string | null>(null);
/** lemma → Anki note id for this session's mines ("open in Anki"). */
export const minedNoteIds = writable<Record<string, number>>({});
/** Lemmas whose note exists but asbplayer media never landed (retry chip). */
export const mediaMissing = writable<Set<string>>(new Set());
/** Gates the mine button — no yomitan-api, no card content. */
export const yomitanReachable = writable(false);
/** Seek/mine lock while asbplayer records the mined line. */
export const playerBusy = writable(false);

/** Must stay in sync with the engine's `anki::mined::normalize_sentence`. */
export const normalizeSentence = (s: string): string => s.replace(/\s+/g, '');

const REFRESH_DEBOUNCE_MS = 5000;
let lastRefresh = 0;

/** Refresh mined/added state from Anki; debounced unless `force`. Silent on
 * failure — Anki being closed must not error on every refocus. */
export async function refreshMinedState(force = false): Promise<void> {
	const now = Date.now();
	if (!force && now - lastRefresh < REFRESH_DEBOUNCE_MS) return;
	lastRefresh = now;
	ipc.getYomitanStatus().then(
		(s) => yomitanReachable.set(s.reachable),
		() => yomitanReachable.set(false)
	);
	try {
		const state = await ipc.getMinedState();
		addedTerms.set(new Set(state.added_terms));
		minedSentences.set(new Set(state.mined_sentences));
		// Backend state covers session mines; keeping the optimistic sets
		// would mask notes deleted in Anki.
		minedTerms.set(new Set());
		sessionMinedSentences.set(new Set());
	} catch {
		// keep the optimistic sets when Anki is unreachable
	}
}

/** One mine: IPC + bookkeeping. No locking, summary toasts, or refresh —
 * `mineTerm` and `mineQueue` own those. The backend waits out the asbplayer
 * recording and verifies the media landed, so the result is definitive. */
async function mineOne(
	term: ipc.Term,
	sentence: string,
	timestamp: ipc.TimeStampDto | null,
	via: 'asbplayer' | 'direct'
): Promise<ipc.MineResult> {
	const result = await ipc.mineTerm(
		{
			term: term.lemma_form,
			sentence,
			timestampSecs: timestamp?.start_secs ?? null,
			timestampEndSecs: timestamp?.end_secs ?? null,
			timestampLabel: timestamp?.start_label ?? null,
			via
		},
		(msg) => {
			if (msg.message) showNotice(msg.message);
		}
	);
	minedTerms.update((s) => new Set(s).add(term.lemma_form));
	if (result.note_id !== null) {
		minedNoteIds.update((m) => ({ ...m, [term.lemma_form]: result.note_id! }));
	}
	if (result.media_missing) {
		mediaMissing.update((s) => new Set(s).add(term.lemma_form));
	}
	if (sentence && result.status === 'created') {
		sessionMinedSentences.update((s) => new Set(s).add(normalizeSentence(sentence)));
	}
	return result;
}

/** Mine one term from its displayed sentence; the caller decides `via`. */
export async function mineTerm(
	term: ipc.Term,
	sentence: string,
	timestamp: ipc.TimeStampDto | null,
	via: 'asbplayer' | 'direct'
): Promise<void> {
	if (get(miningTerm) !== null || get(playerBusy)) return;
	miningTerm.set(term.lemma_form);
	playerBusy.set(true);
	try {
		const result = await mineOne(term, sentence, timestamp, via);
		showNotice(
			result.warning ??
				(result.status === 'duplicate'
					? `「${term.lemma_form}」 is already in Anki`
					: `Added 「${term.lemma_form}」 to Anki`)
		);
		setTimeout(() => void refreshMinedState(true), 2000);
	} catch (err) {
		lastError.set({ title: 'Mining failed', message: String(err), detail: null });
	} finally {
		miningTerm.set(null);
		playerBusy.set(false);
	}
}

/** One selected row, with the occurrence the table displayed at queue time. */
export interface QueueItem {
	term: ipc.Term;
	sentence: string;
	timestamp: ipc.TimeStampDto | null;
}

/** Batch-mine progress (`null` = no queue running). */
export const mineQueueState = writable<{ total: number; done: number; current: string } | null>(
	null
);

let queueCancelled = false;

/** Stops the running queue between items; the in-flight mine still finishes. */
export function cancelQueue(): void {
	queueCancelled = true;
}

/** Mine the items one by one in timestamp order (timestamp-less last).
 * Failures are collected, never abort the queue. */
export async function mineQueue(items: QueueItem[]): Promise<void> {
	if (get(miningTerm) !== null || get(playerBusy) || items.length === 0) return;
	const sorted = [...items].sort((a, b) => {
		const ka = a.timestamp?.start_secs ?? Infinity;
		const kb = b.timestamp?.start_secs ?? Infinity;
		return ka === kb ? 0 : ka - kb;
	});
	queueCancelled = false;
	playerBusy.set(true);
	let created = 0;
	let duplicates = 0;
	let mediaMissed = 0;
	let done = 0;
	const failures: string[] = [];
	try {
		for (const item of sorted) {
			if (queueCancelled) break;
			miningTerm.set(item.term.lemma_form);
			mineQueueState.set({ total: sorted.length, done, current: item.term.lemma_form });
			// Same rule as the single-mine path; re-read per item so an
			// asbplayer disconnect mid-run degrades to direct mines.
			const status = get(playerStatus);
			const via =
				status.mode === 'asbplayer' && status.ws_clients > 0 && item.timestamp !== null
					? 'asbplayer'
					: 'direct';
			try {
				const result = await mineOne(item.term, item.sentence, item.timestamp, via);
				if (result.status === 'duplicate') duplicates++;
				else created++;
				if (result.media_missing) mediaMissed++;
				selectedTerms.update((s) => {
					const next = new Set(s);
					next.delete(termKey(item.term));
					return next;
				});
			} catch (err) {
				failures.push(`「${item.term.lemma_form}」: ${String(err)}`);
			}
			done++;
		}
	} finally {
		miningTerm.set(null);
		playerBusy.set(false);
		mineQueueState.set(null);
		clearSelection();
		void refreshMinedState(true);
		const parts = [`Mined ${created}`];
		if (duplicates > 0) parts.push(`${duplicates} duplicate${duplicates === 1 ? '' : 's'}`);
		if (mediaMissed > 0) parts.push(`${mediaMissed} missing media`);
		if (failures.length > 0) parts.push(`${failures.length} failed`);
		showNotice(
			(queueCancelled && done < sorted.length ? `Cancelled after ${done} — ` : '') +
				parts.join(' · ')
		);
		if (failures.length > 0) {
			lastError.set({
				title: 'Batch mining',
				message: `${failures.length} term${failures.length === 1 ? '' : 's'} failed`,
				detail: failures.join('\n')
			});
		}
	}
}

/** Retry asbplayer enrichment for a media-missing note (session-scoped: needs
 * the note id from this session's mine). */
export async function retryMedia(
	term: ipc.Term,
	timestamp: ipc.TimeStampDto | null
): Promise<void> {
	if (get(miningTerm) !== null || get(playerBusy)) return;
	const noteId = get(minedNoteIds)[term.lemma_form];
	if (noteId === undefined) return;
	miningTerm.set(term.lemma_form);
	playerBusy.set(true);
	try {
		await ipc.retryMineMedia(
			{
				noteId,
				timestampSecs: timestamp?.start_secs ?? null,
				timestampEndSecs: timestamp?.end_secs ?? null,
				timestampLabel: timestamp?.start_label ?? null
			},
			(msg) => {
				if (msg.message) showNotice(msg.message);
			}
		);
		mediaMissing.update((s) => {
			const next = new Set(s);
			next.delete(term.lemma_form);
			return next;
		});
		showNotice(`Added media to 「${term.lemma_form}」`);
	} catch (err) {
		showNotice(`Media retry failed: ${String(err)}`);
	} finally {
		miningTerm.set(null);
		playerBusy.set(false);
	}
}

/** Open Anki's browser on a mined note. */
export async function openInAnki(noteId: number): Promise<void> {
	try {
		await ipc.openInAnki(noteId);
	} catch (err) {
		lastError.set({ title: 'Failed to open Anki', message: String(err), detail: null });
	}
}
