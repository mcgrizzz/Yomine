<script lang="ts" module>
	import type { SentenceDto, Term } from '$lib/ipc';

	/** One appearance of a term: the sentence + the term's UTF-8 byte offset in it. */
	export type Occurrence = { sentence: SentenceDto; start: number };

	const encoder = new TextEncoder();
	const byteLen = (s: string): number => encoder.encode(s).length;
</script>

<script lang="ts">
	// Sentence view (T030, inline): renders a term's example sentence inside its
	// table row with kanji-only `<ruby>` furigana (via Furigana) and the term's
	// own segments highlighted red (egui shows the reading on hover; the web
	// upgrades to furigana). One sentence per row — multi-sentence nav + the
	// comprehension indicator are a deferred US1 sentence-polish follow-up
	// (T030b). The clickable timestamp→seek below is US3/T035.
	import { posColor } from '$lib/pos';
	import { playerConnected, seekTimestamp } from '$lib/stores';
	import Furigana from './Furigana.svelte';

	let { occurrence, term }: { occurrence: Occurrence; term: Term } = $props();

	// The sentence's source timestamp (SRT/ASS only; TXT has none). Clickable when
	// a player is connected → seeks it (egui `ui_timestamp`); otherwise shown as a
	// weak label. The web has no "confirmed" state (PlayerStatus omits it), so the
	// button is always ▶ — egui's 👁 confirmed variant is not mirrored.
	const ts = $derived(occurrence.sentence.timestamp);

	// The term's surface span [start, end) in the sentence (egui uses
	// `full_segment` for expressions, else `surface_form`).
	const isExpression = $derived(
		term.part_of_speech === 'Expression' || term.part_of_speech === 'NounExpression'
	);
	const termEnd = $derived(
		occurrence.start + byteLen(isExpression ? term.full_segment : term.surface_form)
	);

	const isTermSeg = (seg: { start: number; end: number }): boolean =>
		seg.start < termEnd && seg.end > occurrence.start;
</script>

<p class="sentence" lang="ja">
	{#each occurrence.sentence.segments as seg, i (i)}
		{@const isTerm = isTermSeg(seg)}
		<span class:term={isTerm} style="color: {isTerm ? 'var(--red)' : posColor(seg.pos)}"
			><Furigana surface={seg.surface} reading={seg.reading} /></span
		>
	{/each}
</p>

{#if ts}
	{@const t = ts}
	{#if $playerConnected}
		<button
			class="ts"
			title={`Seek to ${t.start_label}`}
			onclick={() => seekTimestamp(t.start_secs, t.start_label)}
		>
			▶ {t.start_label}
		</button>
	{:else}
		<span class="ts-label">{t.start_label}</span>
	{/if}
{/if}

<style>
	.sentence {
		margin: 0;
		font-size: 1.4rem;
		line-height: 2;
	}
	.sentence .term {
		font-weight: 700;
	}

	/* Timestamp meta (T035), below the sentence. Small, like egui's 11px. */
	.ts,
	.ts-label {
		font-size: 0.78rem;
		line-height: 1;
	}
	.ts {
		display: inline-block;
		margin-top: 0.25rem;
		padding: 0.2rem 0.45rem;
		color: var(--green);
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
	}
	.ts:hover {
		background: var(--bg-lighter);
	}
	.ts-label {
		display: inline-block;
		margin-top: 0.25rem;
		color: var(--comment);
	}
</style>
