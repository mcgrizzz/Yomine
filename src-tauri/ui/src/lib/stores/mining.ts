// One-click mining (issue #105), batch mining (issue #114) + already-mined
// state (issue #3).

import { writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError } from './ui';

/** Lemmas mined from the loaded file; this session's mines are added before the backend
 * confirms them. */
export const minedTerms = writable<Set<string>>(new Set());
/** Normalized sentences that already exist in the user's notes. */
export const minedSentences = writable<Set<string>>(new Set());
/** Normalized sentences mined this session (optimistic). */
export const sessionMinedSentences = writable<Set<string>>(new Set());
/** Lemma currently being mined (one mine at a time). */
export const miningTerm = writable<string | null>(null);
/** lemma → Anki note id for this session's mines ("open in Anki"). */
export const minedNoteIds = writable<Record<string, number>>({});
/** Gates the mine button — no yomitan-api, no card content. */
export const yomitanReachable = writable(false);
/** Yomitan term card formats; >1 turns on per-format mine/queue buttons. */
export const cardFormats = writable<ipc.CardFormat[]>([]);
/** Seek/mine lock while asbplayer records the mined line. */
export const playerBusy = writable(false);

/** Must stay in sync with the engine's `anki::mined::normalize_sentence`. */
export const normalizeSentence = (s: string): string => s.replace(/\s+/g, '');

export const isMinedTerm = (t: ipc.Term, mined: Set<string>): boolean => mined.has(t.lemma_form);

export function applyMinedState(state: ipc.MinedState): void {
	// The backend records a note as it's created, so its sets cover this session's mines.
	minedTerms.set(new Set(state.mined_terms));
	minedSentences.set(new Set(state.mined_sentences));
	sessionMinedSentences.set(new Set());
}

const REFRESH_DEBOUNCE_MS = 5000;
let lastRefresh = 0;

/** Reads the mined state and asks for an Anki sync, whose changes arrive as `mined-state`
 * events; debounced unless `force`. */
export async function refreshMinedState(force = false): Promise<void> {
	const now = Date.now();
	if (!force && now - lastRefresh < REFRESH_DEBOUNCE_MS) return;
	lastRefresh = now;
	ipc.getYomitanStatus().then(
		(s) => {
			yomitanReachable.set(s.reachable);
			if (s.reachable) {
				ipc.getCardFormats().then(cardFormats.set, () => cardFormats.set([]));
			} else {
				cardFormats.set([]);
			}
		},
		() => {
			yomitanReachable.set(false);
			cardFormats.set([]);
		}
	);
	void ipc.syncAnki();
	try {
		applyMinedState(await ipc.getMinedState());
	} catch {
		// keep the optimistic sets
	}
}

/** One selected row, with the occurrence the table displayed at queue time. */
export interface QueueItem {
	lemma: string;
	/** `termKey` for a row, a synthetic key for an ad-hoc entry. */
	key: string;
	/** The occurrence text the table highlighted (cloze/bold). */
	surface: string;
	sentence: string;
	timestamp: ipc.TimeStampDto | null;
	/** Yomitan entry chosen via the popover's Queue (default first). */
	entryIndex?: number;
	/** Yomitan card format chosen via the popover's Queue (default first). */
	formatName?: string;
	/** The text the popover scanned — entryIndex is only valid against it. */
	scanText?: string;
}

/** Open Anki's browser on a mined note. */
export async function openInAnki(noteId: number): Promise<void> {
	try {
		await ipc.openInAnki(noteId);
	} catch (err) {
		lastError.set({ title: 'Failed to open Anki', message: String(err), detail: null });
	}
}
