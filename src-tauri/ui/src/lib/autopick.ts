import type { AutoMine, SentenceDto, Term } from '$lib/ipc';
import { harmonic, occurrencesOf, termKey } from '$lib/table';
import { termHighlightText } from '$lib/components/SentenceView.svelte';
import type { QueueItem } from '$lib/stores/mining';

/** 20 points per tenfold increase in commonness: rank 10 → 80, rank 10,000 → 20. */
export const frequencyPoints = (rank: number): number => 20 * Math.log10(100000 / rank);

export function pickPoints(
	rank: number,
	pos: string,
	jlpt: string | null,
	prefs: Pick<AutoMine, 'pos_points' | 'jlpt_points'>
): number {
	return frequencyPoints(rank) + (prefs.pos_points[pos] ?? 0) + (prefs.jlpt_points[jlpt ?? ''] ?? 0);
}

export interface PickOptions {
	prefs: AutoMine;
	isMined: (term: Term) => boolean;
	minedSentences: Set<string>;
	normalize: (sentence: string) => string;
}

// A one-kana "word" is nearly always a misparse fragment (け in ぱんけぇき) or a
// mislabeled particle, yet frequency lists rank that kana highly (で at 8).
const SINGLE_KANA = /^[ぁ-ゖァ-ヺーｦ-ﾟ]$/;

/** Best terms to mine from `terms`, at most one per sentence. */
export function autoPick(terms: Term[], sentences: SentenceDto[], opts: PickOptions): QueueItem[] {
	const ranked = terms
		.filter(
			(t) =>
				harmonic(t) !== Infinity &&
				!opts.isMined(t) &&
				!t.spelling_in_anki &&
				!t.in_speaker_name &&
				!SINGLE_KANA.test(t.lemma_form)
		)
		.map((term) => ({
			term,
			points: pickPoints(harmonic(term), term.part_of_speech, term.jlpt_level, opts.prefs),
			first: Math.min(...term.sentence_references.map(([i]) => i))
		}))
		.sort(
			(a, b) =>
				b.points - a.points ||
				b.term.sentence_references.length - a.term.sentence_references.length ||
				a.first - b.first
		);

	const used = new Set(opts.minedSentences);
	const picks: QueueItem[] = [];
	for (const { term } of ranked) {
		if (picks.length >= opts.prefs.limit) break;
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
