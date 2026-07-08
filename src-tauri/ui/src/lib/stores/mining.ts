// One-click mining (issue #105) + already-mined state (issue #3).

import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError, showNotice } from './ui';

/** Lemmas mined this session (optimistic — set as soon as a mine succeeds). */
export const minedTerms = writable<Set<string>>(new Set());
/** Terms with an Anki card added in the last day (`added:1`), so cards mined
 * outside Yomine (e.g. straight from Yomitan) show up too. */
export const addedTerms = writable<Set<string>>(new Set());
/** Normalized sentences that already exist in the user's notes. */
export const minedSentences = writable<Set<string>>(new Set());
/** Normalized sentences mined this session — shows WHICH sentence a mined term
 * used immediately, without needing a sentence-field mapping. */
export const sessionMinedSentences = writable<Set<string>>(new Set());
/** Lemma currently being mined (busy guard — one mine at a time). */
export const miningTerm = writable<string | null>(null);
/** lemma → Anki note id for this session's mines — powers "open in Anki". */
export const minedNoteIds = writable<Record<string, number>>({});
/** Gates the mine button — no yomitan-api, no card content to mine with. */
export const yomitanReachable = writable(false);
/** True while asbplayer is recording the mined line (cue duration + buffer):
 * seeking or mining mid-recording would ruin the clip, so both are blocked. */
export const playerBusy = writable(false);

/** asbplayer needs a beat after the cue ends to finish encoding + export. */
const RECORD_BUFFER_MS = 1500;

/** Must stay in sync with the engine's `anki::mined::normalize_sentence`
 * (frontend sentences are plain text, so only the whitespace rule applies). */
export const normalizeSentence = (s: string): string => s.replace(/\s+/g, '');

const REFRESH_DEBOUNCE_MS = 5000;
let lastRefresh = 0;

/** Refresh added-term/mined-sentence state from Anki. Debounced for the
 * window-focus trigger; `force` for load/after-mine. Failures are silent —
 * Anki being closed must not surface errors on every refocus. */
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
		// The backend state is authoritative and already covers this session's
		// mines (added:1 + the recorded-mines cache); keeping the optimistic
		// session sets around would mask notes deleted in Anki.
		minedTerms.set(new Set());
		sessionMinedSentences.set(new Set());
	} catch {
		// best-effort — keep the optimistic sets when Anki is unreachable
	}
}

/** Mine one term from its currently-displayed sentence. The caller decides
 * `via` (it knows the session source + player connectivity). */
export async function mineTerm(
	term: ipc.Term,
	sentence: string,
	timestamp: ipc.TimeStampDto | null,
	via: 'asbplayer' | 'direct'
): Promise<void> {
	if (get(miningTerm) !== null || get(playerBusy)) return;
	miningTerm.set(term.lemma_form);
	playerBusy.set(true);
	// Kept past the command's resolution when asbplayer is recording the line.
	let recordHoldMs = 0;
	try {
		const result = await ipc.mineTerm(
			{
				term: term.lemma_form,
				sentence,
				timestampSecs: timestamp?.start_secs ?? null,
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
		if (sentence && result.status === 'created') {
			sessionMinedSentences.update((s) => new Set(s).add(normalizeSentence(sentence)));
		}
		showNotice(
			result.warning ??
				(result.status === 'duplicate'
					? `「${term.lemma_form}」 is already in Anki`
					: `Added 「${term.lemma_form}」 to Anki`)
		);
		// mine-subtitle returns while asbplayer is still recording the cue's
		// audio/screenshot; hold the seek/mine lock for the line's duration.
		if (via === 'asbplayer' && result.status === 'created' && !result.warning && timestamp) {
			const durationMs = Math.max(0, (timestamp.end_secs - timestamp.start_secs) * 1000);
			recordHoldMs = durationMs + RECORD_BUFFER_MS;
		}
		// The asbplayer path exports asynchronously; confirm it landed.
		setTimeout(() => void refreshMinedState(true), 2000 + recordHoldMs);
	} catch (err) {
		lastError.set({ title: 'Mining failed', message: String(err), detail: null });
	} finally {
		miningTerm.set(null);
		if (recordHoldMs > 0) {
			setTimeout(() => playerBusy.set(false), recordHoldMs);
		} else {
			playerBusy.set(false);
		}
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
