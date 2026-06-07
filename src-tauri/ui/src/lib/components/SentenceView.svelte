<script lang="ts" module>
	import type { SentenceDto, Term } from '$lib/ipc';

	/** One appearance of a term: the sentence + the term's UTF-8 byte offset in it. */
	export type Occurrence = { sentence: SentenceDto; start: number };

	const encoder = new TextEncoder();
	const byteLen = (s: string): number => encoder.encode(s).length;
	// CJK ideographs (ext. A + unified + compatibility) — only kanji spans get furigana.
	const hasKanji = (s: string): boolean => /[㐀-鿿豈-﫿]/.test(s);
</script>

<script lang="ts">
	// Sentence view (T030): the term's example sentence with inline `<ruby>`
	// furigana, the term's own segments highlighted (egui shows the reading on
	// hover; the web upgrades to furigana). Browses multiple occurrences.
	import { posColor } from '$lib/pos';

	let { occurrences, term }: { occurrences: Occurrence[]; term: Term } = $props();

	let idx = $state(0);
	const current = $derived(occurrences[Math.min(idx, occurrences.length - 1)]);
	// The term's surface span [start, end) in the current sentence (egui uses
	// `full_segment` for expressions, else `surface_form`).
	const isExpression = $derived(
		term.part_of_speech === 'Expression' || term.part_of_speech === 'NounExpression'
	);
	const termEnd = $derived(
		current ? current.start + byteLen(isExpression ? term.full_segment : term.surface_form) : 0
	);

	const isTermSeg = (seg: { start: number; end: number }): boolean =>
		!!current && seg.start < termEnd && seg.end > current.start;
</script>

{#if current}
	<div class="sentence-view">
		<p class="sentence" lang="ja">
			{#each current.sentence.segments as seg, i (i)}
				{@const isTerm = isTermSeg(seg)}
				{@const color = isTerm ? 'var(--red)' : posColor(seg.pos)}
				{#if hasKanji(seg.surface) && seg.reading && seg.reading !== seg.surface}
					<ruby class:term={isTerm} style="color: {color}"
						>{seg.surface}<rt>{seg.reading}</rt></ruby
					>
				{:else}
					<span class:term={isTerm} style="color: {color}">{seg.surface}</span>
				{/if}
			{/each}
		</p>

		<div class="meta">
			{#if current.sentence.timestamp}
				<span class="time">{current.sentence.timestamp.start_label}</span>
			{/if}
			<span class="comp">{Math.round(current.sentence.comprehension * 100)}% comprehension</span>
			{#if occurrences.length > 1}
				<span class="nav">
					<button onclick={() => (idx = Math.max(0, idx - 1))} disabled={idx === 0}>‹</button>
					{idx + 1}/{occurrences.length}
					<button
						onclick={() => (idx = Math.min(occurrences.length - 1, idx + 1))}
						disabled={idx >= occurrences.length - 1}>›</button
					>
				</span>
			{/if}
		</div>
	</div>
{/if}

<style>
	.sentence-view {
		padding: 0.5rem 0.75rem 0.75rem;
	}
	.sentence {
		margin: 0 0 0.5rem;
		font-size: 1.5rem;
		line-height: 2.1;
	}
	.sentence ruby rt {
		font-size: 0.5em;
		color: var(--comment);
		font-weight: 400;
	}
	.sentence .term {
		font-weight: 700;
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 1rem;
		font-size: 0.8rem;
		color: var(--comment);
	}
	.time {
		color: var(--cyan);
		font-variant-numeric: tabular-nums;
	}
	.nav {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-variant-numeric: tabular-nums;
	}
	.nav button {
		padding: 0 0.4rem;
		line-height: 1.4;
	}
</style>
