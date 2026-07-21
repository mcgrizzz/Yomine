<script lang="ts" module>
	import type { SegmentDto, SentenceDto, Term } from '$lib/ipc';

	/** One appearance of a term: the sentence + the term's UTF-8 byte offset in it. */
	export type Occurrence = { sentence: SentenceDto; start: number };

	export interface SegmentLookup {
		/** Scan text for Yomitan: the sentence from the segment onward, capped. */
		text: string;
		label: string;
		anchor: DOMRect;
		sentence: SentenceDto;
		seg: SegmentDto;
	}

	const encoder = new TextEncoder();
	const byteLen = (s: string): number => encoder.encode(s).length;

	/** Same span rule as `termHighlightText`/`isTermSeg`. */
	export function termCoversSegment(
		term: Term,
		start: number,
		seg: { start: number; end: number }
	): boolean {
		const isExpression =
			term.part_of_speech === 'Expression' || term.part_of_speech === 'NounExpression';
		const end = start + byteLen(isExpression ? term.full_segment : term.surface_form);
		return seg.start < end && seg.end > start;
	}

	/** The text the table highlights for an occurrence: whole segments overlapping
	 * the term's span (must match `isTermSeg` below). Mined cards bold the same. */
	export function termHighlightText(term: Term, occ: Occurrence): string {
		const isExpression =
			term.part_of_speech === 'Expression' || term.part_of_speech === 'NounExpression';
		const end = occ.start + byteLen(isExpression ? term.full_segment : term.surface_form);
		return occ.sentence.segments
			.filter((seg) => seg.start < end && seg.end > occ.start)
			.map((seg) => seg.surface)
			.join('');
	}

	// Filled bars = ceil(pct / 20).
	const BAR_HEIGHTS = [2.5, 4, 6.5, 10.5, 14.5];
</script>

<script lang="ts">
	import { Menu } from '@tauri-apps/api/menu';
	import type { SegmentKnowledge } from '$lib/ipc';
	import { comprehensionColor } from '$lib/comprehension';
	import { furiganaText } from '$lib/furigana';
	import {
		ankiFilterActive,
		minedSentences,
		normalizeSentence,
		playerBusy,
		playerConnected,
		playerStatus,
		seekTimestamp,
		sessionMinedSentences,
		settings
	} from '$lib/stores';
	import Furigana from './Furigana.svelte';

	// No $bindable fallback — Svelte forbids binding undefined (the initial
	// record entry) onto one; reads coalesce to 0 instead.
	let {
		occurrences,
		term,
		currentIndex = $bindable(),
		onlookup,
		onhover
	}: {
		occurrences: Occurrence[];
		term: Term;
		currentIndex?: number;
		onlookup: (req: SegmentLookup) => void;
		/** What to open if Shift is pressed while hovering; null = left. */
		onhover: (open: (() => void) | null) => void;
	} = $props();

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

	// Underlines need Anki data — without the filter everything is unknown and
	// the whole sentence would underline red (same gate as the bars).
	const underlines = $derived(
		($settings?.sentence_coloring ?? 'knowledge') === 'knowledge' && $ankiFilterActive
	);
	const mark = (seg: SegmentDto): SegmentKnowledge | null =>
		underlines && seg.knowledge && ($settings?.sentence_underlines[seg.knowledge] ?? true)
			? seg.knowledge
			: null;

	// Bars only show while Anki filtering is active — without it everything is 0%.
	const comprehensionPct = $derived(occ.sentence.comprehension * 100);
	const filledBars = $derived(Math.min(Math.ceil(comprehensionPct / 20), 5));

	// This exact sentence already lives in an Anki note (issue #3).
	const sentenceMined = $derived.by(() => {
		const key = normalizeSentence(occ.sentence.text);
		return $minedSentences.has(key) || $sessionMinedSentences.has(key);
	});

	// Yomitan scans from the string start, so the remainder of the sentence lets
	// it do its own longest-match + deinflection (e.g. 気 hovers as 気になる).
	function scanText(seg: SegmentDto): string {
		let text = '';
		for (const s of occ.sentence.segments) {
			if (s.start < seg.start) continue;
			text += s.surface;
			if (text.length >= 16) break;
		}
		return text.slice(0, 16);
	}

	// The rt nodes stay inside the selection range even though user-select:none
	// hides them from toString(), so walking it recovers base(reading) pairs.
	function selectionFuriganaText(): string | null {
		const sel = window.getSelection();
		if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
		let out = '';
		const walk = (node: Node, inRt: boolean) => {
			if (node.nodeType === Node.TEXT_NODE) {
				const text = node.textContent ?? '';
				out += inRt && text ? `(${text})` : text;
			} else {
				const rt = inRt || (node as Element).tagName === 'RT';
				node.childNodes.forEach((child) => walk(child, rt));
			}
		};
		for (let i = 0; i < sel.rangeCount; i++) walk(sel.getRangeAt(i).cloneContents(), false);
		return out;
	}

	// Replaces the webview's context menu (which can't be extended) with a native
	// popup; "Copy" re-provides the affordance the suppression removes.
	async function copyMenu(e: MouseEvent) {
		e.preventDefault();
		const sentence = occ.sentence;
		const plain = window.getSelection()?.toString() || sentence.text;
		const ruby =
			selectionFuriganaText() ??
			sentence.segments.map((s) => furiganaText(s.surface, s.reading)).join('');
		const menu = await Menu.new({
			items: [
				{
					id: 'copy',
					text: 'Copy',
					action: () => void navigator.clipboard.writeText(plain)
				},
				{
					id: 'copy-furigana',
					text: 'Copy with furigana',
					action: () => void navigator.clipboard.writeText(ruby)
				}
			]
		});
		await menu.popup();
	}

	function segEnter(e: MouseEvent, seg: SegmentDto) {
		const el = e.currentTarget as HTMLElement;
		const sentence = occ.sentence;
		const open = () =>
			onlookup({
				text: scanText(seg),
				label: seg.surface,
				anchor: el.getBoundingClientRect(),
				sentence,
				seg
			});
		onhover(open);
		if (e.shiftKey) open();
	}
</script>

<!-- Each word is an atomic inline-block and Svelte strips inter-tag whitespace,
     so without the <wbr> the sentence would render as one unbreakable line. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -- right-click copy
     menu is a mouse affordance; keyboard browsing is issue #91. -->
<p class="sentence" lang="ja" oncontextmenu={copyMenu}>
	{#each occ.sentence.segments as seg, i (i)}
		{@const isTerm = isTermSeg(seg)}
		{@const know = mark(seg)}
		{#if i > 0}<wbr />{/if}<!-- svelte-ignore a11y_no_static_element_interactions -- Shift+Hover
			lookup is a mouse-only affordance; keyboard browsing is issue #91. --><span
			class:term={isTerm}
			class:know-unknown={know === 'unknown'}
			class:know-new={know === 'new'}
			class:know-young={know === 'young'}
			class:know-mature={know === 'mature'}
			style="color: {isTerm ? 'var(--danger)' : 'inherit'}"
			onmouseenter={(e) => segEnter(e, seg)}
			onmouseleave={() => onhover(null)}
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
	/* Copyable text — only the paragraph's empty tail row-selects (TermTable). */
	.sentence > span {
		cursor: text;
	}
	.sentence .term {
		font-weight: 700;
	}

	/* Knowledge underlines (issue #94): text color stays free for future pitch
	 * accent. States follow Anki: new = blue, young = orange, mature = green.
	 * Drawn as an inset background bar, not border-bottom — adjacent borders
	 * fuse into one continuous line and word boundaries disappear. */
	.sentence .know-unknown {
		--know-color: var(--know-unknown);
	}
	.sentence .know-new {
		--know-color: var(--know-new);
	}
	.sentence .know-young {
		--know-color: var(--know-young);
	}
	.sentence .know-mature {
		--know-color: var(--know-mature);
	}
	.sentence .know-unknown,
	.sentence .know-new,
	.sentence .know-young,
	.sentence .know-mature {
		background-image: linear-gradient(var(--know-color), var(--know-color));
		background-repeat: no-repeat;
		background-size: calc(100% - 7px) 2.5px;
		background-position: bottom center;
		padding-bottom: 3px;
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
		color: var(--text);
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 2px;
		cursor: pointer;
	}
	.nav-btn:hover:not(:disabled) {
		background: var(--bg-hover);
	}
	.nav-btn:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.counter {
		min-width: 2.2rem;
		text-align: center;
		font-size: 0.7rem;
		color: var(--accent);
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
		color: var(--success);
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
	}
	.ts:hover:not(:disabled) {
		background: var(--bg-hover);
	}
	.ts:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.ts.confirmed {
		background: color-mix(in srgb, var(--success) 55%, var(--bg));
		color: var(--text);
		border-color: color-mix(in srgb, var(--success) 55%, var(--bg));
	}
	.ts.confirmed:hover {
		background: color-mix(in srgb, var(--success) 70%, var(--bg));
	}
	.ts-label {
		color: var(--text-muted);
	}

	.sentence-mined {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.2rem 0.45rem;
		font-size: 0.78rem;
		line-height: 1;
		color: var(--success);
		background: color-mix(in srgb, var(--success) 12%, transparent);
		border: 1px solid color-mix(in srgb, var(--success) 35%, transparent);
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
		background: color-mix(in srgb, var(--text-muted) 30%, transparent);
	}
</style>
