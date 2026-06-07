<script lang="ts">
	// Term table (T029/T030, inline): the US1 mining surface, mirroring egui's
	// four-column layout — Term │ Sentence │ Frequency │ POS. The term shows its
	// reading as furigana above the lemma; the example sentence renders inline in
	// the row (always visible, no expander). Rows arrive already filtered and
	// sorted by `visibleTerms` (driven by `TableControls`, T037).
	import type { SentenceDto, Term } from '$lib/ipc';
	import { harmonic } from '$lib/table';
	import { posCatalog } from '$lib/stores';
	import { posColor } from '$lib/pos';
	import Furigana from './Furigana.svelte';
	import SentenceView, { type Occurrence } from './SentenceView.svelte';

	let { terms, sentences }: { terms: Term[]; sentences: SentenceDto[] } = $props();

	// key → display label ("Postposition" → "Particle"), from get_pos_catalog.
	const posLabels = $derived(Object.fromEntries($posCatalog.map((p) => [p.key, p.display_name])));

	// `Term.id` is not unique (the engine doesn't assign distinct ids); the pipeline
	// dedups by (lemma_form, hiragana reading), so this pair is a stable unique key.
	const termKey = (t: Term): string => `${t.lemma_form} ${t.lemma_reading}`;

	function freqLabel(term: Term): string {
		const v = harmonic(term);
		return v === Infinity ? '？' : String(v);
	}

	// Resolve a term's first `sentence_reference` (index, byte offset) to its
	// example sentence, matching egui's index-based lookup into the sentences
	// array. Multi-sentence nav is a deferred US1 follow-up, so we show the first.
	function firstOccurrence(term: Term): Occurrence | undefined {
		for (const [i, start] of term.sentence_references) {
			const sentence = sentences[i];
			if (sentence) return { sentence, start };
		}
		return undefined;
	}
</script>

<div class="table">
	<div class="row head">
		<span>Term</span>
		<span>Sentence</span>
		<span class="num">Frequency</span>
		<span>POS</span>
	</div>
	{#each terms as term (termKey(term))}
		{@const occ = firstOccurrence(term)}
		<div class="row">
			<span class="term" lang="ja"><Furigana surface={term.lemma_form} reading={term.lemma_reading} /></span>
			<div class="sentence">
				{#if occ}
					<SentenceView occurrence={occ} {term} />
				{:else}
					<span class="empty">—</span>
				{/if}
			</div>
			<span class="num">{freqLabel(term)}</span>
			<span class="pos" style="color: {posColor(term.part_of_speech)}">
				{posLabels[term.part_of_speech] ?? term.part_of_speech}
			</span>
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
		grid-template-columns: minmax(7rem, max-content) 1fr 6rem 8rem;
		gap: 0.75rem;
		align-items: center;
		padding: 0.5rem;
		border-bottom: 1px solid var(--border);
	}
	.row:not(.head):hover {
		background: var(--bg-light);
	}
	.row.head {
		position: sticky;
		top: 0;
		background: var(--bg-dark);
		color: var(--comment);
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	.num {
		text-align: right;
	}
	.term {
		font-size: 1.5rem;
		color: var(--red);
		line-height: 1.1;
	}
	.pos {
		font-size: 0.9rem;
	}
	.empty {
		color: var(--comment);
	}
</style>
