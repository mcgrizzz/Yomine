<script lang="ts" module>
	import type { SentenceDto, Term } from '$lib/ipc';

	/** One appearance of a term: the sentence + the term's UTF-8 byte offset in it. */
	export type Occurrence = { sentence: SentenceDto; start: number };

	const encoder = new TextEncoder();
	const byteLen = (s: string): number => encoder.encode(s).length;

	// egui's 5-bar indicator: fixed bar heights, filled = ceil(pct / 20).
	const BAR_HEIGHTS = [2.5, 4, 6.5, 10.5, 14.5];
</script>

<script lang="ts">
	// Sentence view (T030/T030b, inline): renders a term's example sentences
	// inside its table row with kanji-only `<ruby>` furigana (via Furigana) and
	// the term's own segments highlighted red (egui shows the reading on hover;
	// the web upgrades to furigana). Below the sentence sits the meta row
	// (egui `sentence_column.rs`): ◀ n/m ▶ nav across the term's occurrences
	// (wrap-around, like `TableState::next/prev_sentence`), the timestamp
	// (US3/T035), and the per-sentence comprehension indicator (5-bar gradient,
	// only while Anki filtering is active).
	import { comprehensionColor } from '$lib/comprehension';
	import { posColor } from '$lib/pos';
	import { ankiFilterActive, playerConnected, playerStatus, seekTimestamp } from '$lib/stores';
	import Furigana from './Furigana.svelte';

	let { occurrences, term }: { occurrences: Occurrence[]; term: Term } = $props();

	// Which occurrence is shown. Local per-row state (egui keeps it in
	// `table_state.sentence_indices`); clamped in case a refresh shrinks the list.
	let idx = $state(0);
	const count = $derived(occurrences.length);
	const current = $derived(Math.min(idx, count - 1));
	const occ = $derived(occurrences[current]);

	const prev = () => (idx = current === 0 ? count - 1 : current - 1);
	const next = () => (idx = (current + 1) % count);

	// The sentence's source timestamp (SRT/ASS only; TXT has none). Clickable when
	// a player is connected → seeks it (egui `ui_timestamp`); otherwise shown as a
	// weak label. Once the player acknowledges a seek, the button flips to egui's
	// 👁 confirmed variant with a green fill (T063; `ui_timestamp_button`).
	const ts = $derived(occ.sentence.timestamp);
	const confirmed = $derived(
		ts !== null && $playerStatus.confirmed_timestamps.includes(ts.start_secs)
	);

	// The term's surface span [start, end) in the sentence (egui uses
	// `full_segment` for expressions, else `surface_form`).
	const isExpression = $derived(
		term.part_of_speech === 'Expression' || term.part_of_speech === 'NounExpression'
	);
	const termEnd = $derived(
		occ.start + byteLen(isExpression ? term.full_segment : term.surface_form)
	);

	const isTermSeg = (seg: { start: number; end: number }): boolean =>
		seg.start < termEnd && seg.end > occ.start;

	// Comprehension of the shown sentence, as a % (egui hides the bars until
	// Anki filtering is active — without it every sentence reads 0%).
	const comprehensionPct = $derived(occ.sentence.comprehension * 100);
	const filledBars = $derived(Math.min(Math.ceil(comprehensionPct / 20), 5));
</script>

<!-- T030c: each word is an atomic inline-block (furigana can't overhang its
     neighbour), and Svelte strips inter-tag whitespace — so without an explicit
     break opportunity the sentence renders as one unbreakable line. The <wbr>
     between word boxes restores soft wrap between words (egui parity:
     `ui.horizontal_wrapped`). -->
<p class="sentence" lang="ja">
	{#each occ.sentence.segments as seg, i (i)}
		{@const isTerm = isTermSeg(seg)}
		{#if i > 0}<wbr />{/if}<span
			class:term={isTerm}
			style="color: {isTerm ? 'var(--red)' : posColor(seg.pos)}"
			><Furigana surface={seg.surface} reading={seg.reading} /></span
		>
	{/each}
</p>

<div class="meta">
	<span class="nav">
		<button type="button" class="nav-btn" disabled={count <= 1} title="Previous sentence" onclick={prev}
			>⏮</button
		>
		<span class="counter">{current + 1}/{count}</span>
		<button type="button" class="nav-btn" disabled={count <= 1} title="Next sentence" onclick={next}
			>⏭</button
		>
	</span>

	{#if ts}
		{@const t = ts}
		{#if $playerConnected}
			<button
				class="ts"
				class:confirmed
				title={`Seek to ${t.start_label}`}
				onclick={() => seekTimestamp(t.start_secs, t.start_label)}
			>
				{confirmed ? '👁' : '▶'} {t.start_label}
			</button>
		{:else}
			<span class="ts-label">{t.start_label}</span>
		{/if}
	{/if}

	{#if $ankiFilterActive}
		<span class="bars" title={`${comprehensionPct.toFixed(0)}% comprehensibility`}>
			{#each BAR_HEIGHTS as h, i (i)}
				<span
					class="bar"
					class:empty={i >= filledBars}
					style="height: {h}px; {i < filledBars
						? `background: ${comprehensionColor(comprehensionPct)};`
						: ''}"
				></span>
			{/each}
		</span>
	{/if}
</div>

<style>
	.sentence {
		margin: 0;
		font-size: 1.4rem;
		line-height: 2;
	}
	.sentence .term {
		font-weight: 700;
	}

	/* Meta row under the sentence (egui's nav + timestamp + comprehension line). */
	.meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.25rem;
	}

	/* ◀ n/m ▶ multi-sentence nav (egui `ui_sentence_navigation`). */
	.nav {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
	}
	.nav-btn {
		padding: 0.1rem 0.35rem;
		font-size: 0.7rem;
		line-height: 1.2;
		color: var(--fg);
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: 2px;
		cursor: pointer;
	}
	.nav-btn:hover:not(:disabled) {
		background: var(--bg-lighter);
	}
	.nav-btn:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.counter {
		min-width: 2.2rem;
		text-align: center;
		font-size: 0.7rem;
		color: var(--cyan);
		font-variant-numeric: tabular-nums;
	}

	/* Timestamp meta (T035). Small, like egui's 11px. */
	.ts,
	.ts-label {
		font-size: 0.78rem;
		line-height: 1;
	}
	.ts {
		display: inline-block;
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
	/* Player-acknowledged seek: egui's 👁 button fill (#559449), white text. */
	.ts.confirmed {
		background: #559449;
		color: #fff;
		border-color: #559449;
	}
	.ts.confirmed:hover {
		background: color-mix(in srgb, #559449 85%, white);
	}
	.ts-label {
		color: var(--comment);
	}

	/* Per-sentence comprehension (egui `ui_sentence_comprehension`): 5 bars of
	   growing height, filled from the left, red→yellow→green by %. */
	.bars {
		display: inline-flex;
		align-items: flex-end;
		gap: 1px;
		height: 16px;
	}
	.bar {
		width: 3px;
		border-radius: 1px;
	}
	.bar.empty {
		background: color-mix(in srgb, var(--comment) 30%, transparent);
	}
</style>
