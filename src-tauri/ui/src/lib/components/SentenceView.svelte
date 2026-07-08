<script lang="ts" module>
	import type { SentenceDto, Term } from '$lib/ipc';

	/** One appearance of a term: the sentence + the term's UTF-8 byte offset in it. */
	export type Occurrence = { sentence: SentenceDto; start: number };

	const encoder = new TextEncoder();
	const byteLen = (s: string): number => encoder.encode(s).length;

	// Filled bars = ceil(pct / 20).
	const BAR_HEIGHTS = [2.5, 4, 6.5, 10.5, 14.5];
</script>

<script lang="ts">
	import { comprehensionColor } from '$lib/comprehension';
	import { posColor } from '$lib/pos';
	import {
		ankiFilterActive,
		minedSentences,
		normalizeSentence,
		playerBusy,
		playerConnected,
		playerStatus,
		seekTimestamp,
		sessionMinedSentences
	} from '$lib/stores';
	import Furigana from './Furigana.svelte';

	// No $bindable fallback — Svelte forbids binding undefined (the initial
	// record entry) onto one; reads coalesce to 0 instead.
	let {
		occurrences,
		term,
		currentIndex = $bindable()
	}: { occurrences: Occurrence[]; term: Term; currentIndex?: number } = $props();

	// Clamped in case a refresh shrinks the occurrence list.
	const count = $derived(occurrences.length);
	const current = $derived(Math.min(currentIndex ?? 0, count - 1));
	const occ = $derived(occurrences[current]);

	const prev = () => (currentIndex = current === 0 ? count - 1 : current - 1);
	const next = () => (currentIndex = (current + 1) % count);

	// Timestamp is null for TXT sources; 👁 once the player acknowledges the seek.
	const ts = $derived(occ.sentence.timestamp);
	const confirmed = $derived(
		ts !== null && $playerStatus.confirmed_timestamps.includes(ts.start_secs)
	);

	// Expressions highlight their full segment; other terms just the surface form.
	const isExpression = $derived(
		term.part_of_speech === 'Expression' || term.part_of_speech === 'NounExpression'
	);
	const termEnd = $derived(
		occ.start + byteLen(isExpression ? term.full_segment : term.surface_form)
	);

	const isTermSeg = (seg: { start: number; end: number }): boolean =>
		seg.start < termEnd && seg.end > occ.start;

	// Bars only show while Anki filtering is active — without it everything is 0%.
	const comprehensionPct = $derived(occ.sentence.comprehension * 100);
	const filledBars = $derived(Math.min(Math.ceil(comprehensionPct / 20), 5));

	// This exact sentence already lives in an Anki note (issue #3).
	const sentenceMined = $derived.by(() => {
		const key = normalizeSentence(occ.sentence.text);
		return $minedSentences.has(key) || $sessionMinedSentences.has(key);
	});
</script>

<!-- Each word is an atomic inline-block and Svelte strips inter-tag whitespace,
     so without the <wbr> the sentence would render as one unbreakable line. -->
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
				disabled={$playerBusy}
				title={$playerBusy
					? 'Waiting for asbplayer to finish recording the mined line…'
					: `Seek to ${t.start_label}`}
				onclick={() => seekTimestamp(t.start_secs, t.start_label)}
			>
				{confirmed ? '👁' : '▶'} {t.start_label}
			</button>
		{:else}
			<span class="ts-label">{t.start_label}</span>
		{/if}
	{/if}

	{#if sentenceMined}
		<span class="sentence-mined" title="This sentence is already in one of your Anki notes"
			>✓</span
		>
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

	.meta {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.25rem;
	}

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
	.ts:hover:not(:disabled) {
		background: var(--bg-lighter);
	}
	.ts:disabled {
		opacity: 0.5;
		cursor: default;
	}
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

	.sentence-mined {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.2rem 0.45rem;
		font-size: 0.78rem;
		line-height: 1;
		color: var(--green);
		background: color-mix(in srgb, var(--green) 12%, transparent);
		border: 1px solid color-mix(in srgb, var(--green) 35%, transparent);
		border-radius: var(--radius);
		cursor: help;
	}

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
