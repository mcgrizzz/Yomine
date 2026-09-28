import type { AutoMine, SentenceDto, Term } from '$lib/ipc';
import { harmonic, occurrencesOf, termKey } from '$lib/table';
import { termHighlightText } from '$lib/components/SentenceView.svelte';
import type { QueueItem } from '$lib/stores/mining';

/** The horizon before the user's Anki coverage is known, about where a beginner's is. */
export const DEFAULT_HORIZON = 1000;
const FULL_FREQUENCY_POINTS = 40;

/** Full points up to the horizon, where the user's knowledge reaches (`KnowledgeSummary`),
 * then 20 fewer per tenfold step past it. */
export const frequencyPoints = (rank: number, horizon: number): number =>
	FULL_FREQUENCY_POINTS - 20 * Math.max(0, Math.log10(rank / horizon));

/** Word types the UI shows under another's name (both read "Compound Noun"), scored as it. */
export const POS_ALIASES: Record<string, string> = { NounExpression: 'CompoundNoun' };

export interface ScoreParts {
	frequency: number;
	wordType: number;
	jlpt: number;
}

export function scoreParts(
	rank: number,
	pos: string,
	jlpt: string | null,
	horizon: number,
	prefs: Pick<AutoMine, 'pos_points' | 'jlpt_points'>
): ScoreParts {
	return {
		frequency: frequencyPoints(rank, horizon),
		wordType: prefs.pos_points[POS_ALIASES[pos] ?? pos] ?? 0,
		jlpt: prefs.jlpt_points[jlpt ?? ''] ?? 0
	};
}

export function pickPoints(
	rank: number,
	pos: string,
	jlpt: string | null,
	horizon: number,
	prefs: Pick<AutoMine, 'pos_points' | 'jlpt_points'>
): number {
	const parts = scoreParts(rank, pos, jlpt, horizon, prefs);
	return parts.frequency + parts.wordType + parts.jlpt;
}

export interface PickOptions {
	prefs: AutoMine;
	horizon: number;
	isMined: (term: Term) => boolean;
	minedSentences: Set<string>;
	normalize: (sentence: string) => string;
}

const SINGLE_KANA = /^[ぁ-ゖァ-ヺーｦ-ﾟ]$/;

/** Terms auto mode never picks; the table still lists them. */
function skipped(t: Term, opts: PickOptions): boolean {
	return (
		// No frequency list ranks it, so it's likely a misparse or a name.
		harmonic(t) === Infinity ||
		opts.isMined(t) ||
		// A one-kana "word" is nearly always a misparse fragment (け in ぱんけぇき) or a
		// mislabeled particle, yet frequency lists rank that kana highly (で at 8).
		SINGLE_KANA.test(t.lemma_form) ||
		Object.values(t.auto_skip ?? {}).some(Boolean)
	);
}

export interface AutoPick {
	item: QueueItem;
	/** The chosen sentence, as an index into the term's occurrences. */
	occIdx: number;
}

export type MissReason = 'limit' | 'min_score' | 'sentence_used';

export interface NextInLine {
	term: Term;
	reason: MissReason;
}

const NEXT_IN_LINE = 5;

/** Best terms to mine from `terms`, at most one per sentence, and the few ranked after them. */
export function autoPick(
	terms: Term[],
	sentences: SentenceDto[],
	opts: PickOptions
): { picks: AutoPick[]; next: NextInLine[] } {
	const ranked = terms
		.filter((t) => !skipped(t, opts))
		.map((term) => ({
			term,
			points: pickPoints(
				harmonic(term),
				term.part_of_speech,
				term.jlpt_level,
				opts.horizon,
				opts.prefs
			),
			first: Math.min(...term.sentence_references.map(([i]) => i))
		}))
		// Words inside the horizon tie on frequency points, so the commoner goes first.
		.sort(
			(a, b) =>
				b.points - a.points ||
				harmonic(a.term) - harmonic(b.term) ||
				b.term.sentence_references.length - a.term.sentence_references.length ||
				a.first - b.first
		);

	const used = new Set(opts.minedSentences);
	const picks: AutoPick[] = [];
	const next: NextInLine[] = [];
	const { stop, limit, min_score, max_cards } = opts.prefs;
	const cap = stop === 'count' ? limit : (max_cards ?? Infinity);
	for (const { term, points } of ranked) {
		// Ranked best first: once one is under the minimum, the rest are too.
		const low = stop === 'min_score' && points < min_score;
		if (low || picks.length >= cap) {
			if (next.length >= NEXT_IN_LINE) break;
			next.push({ term, reason: low ? 'min_score' : 'limit' });
			continue;
		}
		const occs = occurrencesOf(term, sentences);
		let best = -1;
		occs.forEach((o, i) => {
			if (used.has(opts.normalize(o.sentence.text))) return;
			if (best < 0 || o.sentence.comprehension > occs[best].sentence.comprehension) best = i;
		});
		if (best < 0) {
			if (next.length < NEXT_IN_LINE) next.push({ term, reason: 'sentence_used' });
			continue;
		}
		const occ = occs[best];
		used.add(opts.normalize(occ.sentence.text));
		picks.push({
			item: {
				lemma: term.lemma_form,
				key: termKey(term),
				surface: termHighlightText(term, occ),
				sentence: occ.sentence.text,
				timestamp: occ.sentence.timestamp
			},
			occIdx: best
		});
	}
	return { picks, next };
}
