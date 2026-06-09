// Client-side term-table controls (T037): search, sort, POS filter, and a
// frequency range — pure functions over the term list, mirroring egui's
// `gui/table/{sort,filter,search}.rs` (research R6: all client-side on the
// loaded term set, no per-keystroke round trip).

import { isHiragana, toHiragana } from 'wanakana';
import type { SentenceDto, Term } from '$lib/ipc';

/** Sentinel the engine uses for "no frequency data" (u32::MAX). */
export const NO_FREQ = 4294967295;

/**
 * Combined (weighted-harmonic) frequency rank; the engine stores it under the
 * `"HARMONIC"` key (`FrequencyManager::get_weighted_harmonic`). Missing or the
 * sentinel → `Infinity`, so it sorts last and is treated as "unknown" by the
 * frequency filter — matching egui's `weighted_frequency`.
 */
export function harmonic(term: Term): number {
	const v = term.frequencies.HARMONIC;
	return v === undefined || v === NO_FREQ ? Infinity : v;
}

export type SortField = 'frequency' | 'chronological' | 'sentenceCount' | 'comprehension';
export type SortDir = 'asc' | 'desc';

export const SORT_FIELDS: { value: SortField; label: string }[] = [
	{ value: 'frequency', label: 'Frequency' },
	{ value: 'chronological', label: 'Chronological' },
	{ value: 'sentenceCount', label: 'Sentence count' },
	{ value: 'comprehension', label: 'Comprehension' }
];

/** egui `SortState::default_direction`: frequency/chronological ascending; count/comprehension descending. */
export function defaultDir(field: SortField): SortDir {
	return field === 'frequency' || field === 'chronological' ? 'asc' : 'desc';
}

/** Earliest sentence index the term appears in (egui Chronological key); none → +Infinity. */
function chronoIndex(term: Term): number {
	let min = Infinity;
	for (const [idx] of term.sentence_references) if (idx < min) min = idx;
	return min;
}

/** Comprehension of the term's first resolvable sentence — the one the table shows. */
function comprehensionOf(term: Term, sentences: SentenceDto[]): number {
	for (const [idx] of term.sentence_references) {
		const s = sentences[idx];
		if (s) return s.comprehension;
	}
	return 0;
}

/** Selected frequency window + whether unknown-frequency terms are shown. */
export interface FreqRange {
	min: number;
	max: number;
	includeUnknown: boolean;
}

export interface TableControlState {
	search: string;
	sort: { field: SortField; dir: SortDir };
	/** POS-key → enabled; a missing key counts as enabled (egui `is_enabled` default). */
	pos: Record<string, boolean>;
	freq: FreqRange | null;
}

/**
 * Min/max of the known (non-unknown) harmonic frequencies, mirroring egui's
 * `configure_bounds` + `update_bounds` clamping (lower bound floored at 1).
 */
export function freqBounds(terms: Term[]): { min: number; max: number } {
	let min = Infinity;
	let max = 0;
	for (const t of terms) {
		const f = harmonic(t);
		if (f === Infinity) continue;
		if (f < min) min = f;
		if (f > max) max = f;
	}
	if (min === Infinity) min = 0;
	const lo = Math.max(min, 1);
	const hi = Math.max(max, lo);
	return { min: lo, max: hi };
}

// o-row+お→う / e-row+え→い, mirroring `core::utils::NormalizeLongVowel`
// (とおい→とうい; けいたい unchanged). Applied only to all-hiragana strings.
const LONG_VOWEL_RE = /([おこそとのほもよろごぞどぼぽ])お|([けせてねへめれげぜでべぺ])え/g;

/** egui `str::normalize_long_vowel`: no-op unless the whole string is hiragana. */
function normalizeLongVowel(s: string): string {
	if (!isHiragana(s)) return s;
	return s.replace(LONG_VOWEL_RE, (_m, oRow, eRow) => (oRow ? oRow + 'う' : eRow + 'い'));
}

/**
 * egui `core::utils::normalize_japanese_text`: convert to hiragana via wana-kana
 * (romaji → kana, katakana → kana; kanji/other pass through), then fold long
 * vowels. Uses the `wanakana` JS package — the library `wana_kana` (Rust) was
 * ported from — so the conversion matches the engine's.
 */
function normalizeJapaneseText(text: string): string {
	return normalizeLongVowel(toHiragana(text));
}

/**
 * Substring match, a faithful port of `core::utils::text_matches_search`: first
 * the normalized-Japanese pass (romaji/katakana → hiragana + long-vowel fold),
 * then a case-insensitive ASCII fallback (e.g. English POS names).
 */
export function textMatches(text: string, query: string): boolean {
	if (normalizeJapaneseText(text).includes(normalizeJapaneseText(query))) return true;
	return text.toLowerCase().includes(query.toLowerCase());
}

/** egui `search::matches_search`: term forms/readings/POS, then sentence text. */
export function matchesSearch(term: Term, sentences: SentenceDto[], query: string): boolean {
	const q = query.trim();
	if (q === '') return true;
	if (
		textMatches(term.lemma_form, q) ||
		textMatches(term.surface_form, q) ||
		textMatches(term.full_segment, q) ||
		textMatches(term.lemma_reading, q) ||
		textMatches(term.surface_reading, q) ||
		textMatches(term.part_of_speech, q)
	) {
		return true;
	}
	for (const [idx] of term.sentence_references) {
		const s = sentences[idx];
		if (s && textMatches(s.text, q)) return true;
	}
	return false;
}

/** Filter then sort the term list per the current controls, mirroring egui `recompute_indices`. */
export function applyControls(
	terms: Term[],
	sentences: SentenceDto[],
	c: TableControlState
): Term[] {
	const out = terms.filter((t) => {
		// Frequency range (egui `FrequencyFilter::contains`).
		if (c.freq) {
			const f = harmonic(t);
			if (f === Infinity) {
				if (!c.freq.includeUnknown) return false;
			} else if (f < c.freq.min || f > c.freq.max) {
				return false;
			}
		}
		// POS filter (egui `is_enabled`: missing key → enabled).
		if (c.pos[t.part_of_speech] === false) return false;
		// Search.
		return matchesSearch(t, sentences, c.search);
	});

	const { field, dir } = c.sort;
	const keyOf = (t: Term): number => {
		switch (field) {
			case 'frequency':
				return harmonic(t);
			case 'chronological':
				return chronoIndex(t);
			case 'sentenceCount':
				return t.sentence_references.length;
			case 'comprehension':
				return comprehensionOf(t, sentences);
		}
	};
	out.sort((a, b) => {
		const ka = keyOf(a);
		const kb = keyOf(b);
		const d = ka === kb ? 0 : ka - kb; // guards Infinity − Infinity = NaN
		return dir === 'asc' ? d : -d;
	});
	return out;
}
