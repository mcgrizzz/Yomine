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

export function pickPoints(
	rank: number,
	pos: string,
	jlpt: string | null,
	horizon: number,
	prefs: Pick<AutoMine, 'pos_points' | 'jlpt_points'>
): number {
	return (
		frequencyPoints(rank, horizon) +
		(prefs.pos_points[POS_ALIASES[pos] ?? pos] ?? 0) +
		(prefs.jlpt_points[jlpt ?? ''] ?? 0)
	);
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

/** Best terms to mine from `terms`, at most one per sentence. */
export function autoPick(terms: Term[], sentences: SentenceDto[], opts: PickOptions): QueueItem[] {
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
	const picks: QueueItem[] = [];
	const { stop, limit, min_score, max_cards } = opts.prefs;
	const cap = stop === 'count' ? limit : (max_cards ?? Infinity);
	for (const { term, points } of ranked) {
		// Ranked best first, so the first term under the minimum ends the picks.
		if (picks.length >= cap || (stop === 'min_score' && points < min_score)) break;
		const occ = occurrencesOf(term, sentences)
			.filter((o) => !used.has(opts.normalize(o.sentence.text)))
			.reduce<ReturnType<typeof occurrencesOf>[number] | null>(
				(best, o) => (!best || o.sentence.comprehension > best.sentence.comprehension ? o : best),
				null
			);
		if (!occ) continue;
		used.add(opts.normalize(occ.sentence.text));
		picks.push({
			lemma: term.lemma_form,
			key: termKey(term),
			surface: termHighlightText(term, occ),
			sentence: occ.sentence.text,
			timestamp: occ.sentence.timestamp
		});
	}
	return picks;
}
