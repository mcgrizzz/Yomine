// One-click mining (issue #105) + already-mined state (issue #3).

import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
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
		// The backend waits out the asbplayer recording and verifies the media
		// landed, so the result is definitive by the time it resolves.
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
