// One-click mining (issue #105), batch mining (issue #114) + already-mined
// state (issue #3).

import { writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { playerStatus } from './player';
import { lastError } from './ui';

/** Lemmas mined this session (optimistic, until the next refresh). */
export const minedTerms = writable<Set<string>>(new Set());
/** Terms with an Anki card added in the last day (`added:1`). */
export const addedTerms = writable<Set<string>>(new Set());
/** `entry_key`s for those same notes; covers what the vocab cache is too old for. */
export const addedKeys = writable<Set<string>>(new Set());
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

export const isMinedTerm = (t: ipc.Term, mined: Set<string>, added: Set<string>): boolean =>
	mined.has(t.lemma_form) || added.has(t.lemma_form) || added.has(t.surface_form);

const REFRESH_DEBOUNCE_MS = 5000;
let lastRefresh = 0;

/** Refresh mined/added state from Anki; debounced unless `force`. Silent on
 * failure — Anki being closed must not error on every refocus. */
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
	try {
		const state = await ipc.getMinedState();
		addedTerms.set(new Set(state.added_terms));
		addedKeys.set(new Set(state.added_keys));
		minedSentences.set(new Set(state.mined_sentences));
		// Backend state covers session mines; keeping the optimistic sets
		// would mask notes deleted in Anki.
		minedTerms.set(new Set());
		sessionMinedSentences.set(new Set());
	} catch {
		// keep the optimistic sets when Anki is unreachable
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
