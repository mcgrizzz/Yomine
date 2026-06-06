<script lang="ts" module>
	import type { Term } from '$lib/ipc';

	/** Sentinel the engine uses for "no frequency data" (u32::MAX). */
	const NO_FREQ = 4294967295;

	/** Combined frequency rank; missing/sentinel sorts last. Reused by T037 sort. */
	export function harmonic(term: Term): number {
		const v = term.frequencies.HARMONIC;
		return v === undefined || v === NO_FREQ ? Infinity : v;
	}
</script>

<script lang="ts">
	// Term table (T029): the US1 mining surface. Rows are sorted by frequency
	// (ascending rank = most frequent first), mirroring egui's default. Interactive
	// sort/filter/search land in T037; the expandable furigana sentence view in T030.
	// `Term` is imported by the module script above (shared scope).
	import { posCatalog } from '$lib/stores';

	/** POS-key → CSS color token, mirroring egui `Theme::pos_color` groups. */
	function posColorVar(posKey: string): string {
		switch (posKey) {
			case 'Verb':
			case 'SuruVerb':
				return 'var(--pos-verb)';
			case 'Noun':
				return 'var(--pos-noun)';
			case 'Adjective':
			case 'AdjectivalNoun':
				return 'var(--pos-adjective)';
			case 'Adverb':
				return 'var(--pos-adverb)';
			case 'Postposition':
				return 'var(--pos-particle)';
			default:
				return 'var(--pos-other)';
		}
	}

	let { terms }: { terms: Term[] } = $props();

	// key → display label ("Postposition" → "Particle"), from get_pos_catalog.
	const posLabels = $derived(
		Object.fromEntries($posCatalog.map((p) => [p.key, p.display_name]))
	);

	const sorted = $derived([...terms].sort((a, b) => harmonic(a) - harmonic(b)));

	function freqLabel(term: Term): string {
		const v = harmonic(term);
		return v === Infinity ? '？' : String(v);
	}
</script>

<div class="table" role="table">
	<div class="row head" role="row">
		<span role="columnheader">Term</span>
		<span role="columnheader">POS</span>
		<span class="num" role="columnheader">Frequency</span>
		<span class="num" role="columnheader">Sentences</span>
		<span class="num" role="columnheader">Comp.</span>
	</div>
	{#each sorted as term (term.id)}
		<div class="row" role="row">
			<span class="term" role="cell">
				<span class="lemma">{term.lemma_form}</span>
				<span class="reading">{term.lemma_reading}</span>
			</span>
			<span class="pos" style="color: {posColorVar(term.part_of_speech)}" role="cell">
				{posLabels[term.part_of_speech] ?? term.part_of_speech}
			</span>
			<span class="num" role="cell">{freqLabel(term)}</span>
			<span class="num" role="cell">{term.sentence_references.length}</span>
			<span class="num" role="cell">{Math.round(term.comprehension * 100)}%</span>
		</div>
	{/each}
</div>

<style>
	.table {
		display: flex;
		flex-direction: column;
		font-variant-numeric: tabular-nums;
	}
	.row {
		display: grid;
		grid-template-columns: 2fr 1fr 6rem 6rem 5rem;
		gap: 0.75rem;
		align-items: baseline;
		padding: 0.35rem 0.5rem;
		border-bottom: 1px solid var(--border);
	}
	.row.head {
		position: sticky;
		top: 0;
		background: var(--bg-dark);
		color: var(--comment);
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		border-bottom: 1px solid var(--border);
	}
	.num {
		text-align: right;
	}
	.term {
		display: flex;
		flex-direction: column;
		line-height: 1.2;
	}
	.lemma {
		font-size: 1.3rem;
		color: var(--red);
	}
	.reading {
		font-size: 0.8rem;
		color: var(--comment);
	}
	.pos {
		font-size: 0.9rem;
	}
</style>
