<script module lang="ts">
	import type { Term, TimeStampDto } from '$lib/ipc';

	export interface OccurrenceAlt {
		/** Index into the row's occurrence list (syncs `occIdx` on reassign). */
		idx: number;
		sentence: string;
		timestamp: TimeStampDto | null;
		/** The occurrence text the table highlighted (cloze/bold). */
		surface: string;
	}

	/** One queued term with what's needed to resolve sentence conflicts. */
	export interface BatchEntry {
		/** Absent for an ad-hoc entry: no table row backs it. */
		term?: Term;
		lemma: string;
		key: string;
		/** The occurrence text the table highlighted (cloze/bold). */
		surface: string;
		sentence: string;
		timestamp: TimeStampDto | null;
		/** Yomitan entry chosen via the popover's Queue (default first). */
		entryIndex?: number;
		/** Yomitan card format chosen via the popover's Queue (default first). */
		formatName?: string;
		/** The text the popover scanned — entryIndex is only valid against it. */
		scanText?: string;
		/** The user explicitly navigated to this occurrence. */
		explicit: boolean;
		/** Occurrences with a different sentence than the chosen one. */
		alternatives: OccurrenceAlt[];
	}
</script>

<script lang="ts">
	import Modal from './Modal.svelte';
	import { harmonic } from '$lib/table';
	import { DEFAULT_HORIZON, pickPoints } from '$lib/autopick';
	import { knowledge, normalizeSentence, settings, type QueueItem } from '$lib/stores';

	let {
		entries,
		ondone,
		oncancel,
		onlookup,
		onhover
	}: {
		entries: BatchEntry[];
		ondone: (items: QueueItem[], occIdxPatch: Record<string, number>) => void;
		oncancel: () => void;
		onlookup?: (req: { text: string; label: string; anchor: DOMRect }) => void;
		onhover?: (fn: (() => void) | null) => void;
	} = $props();

	const skey = (s: string) => normalizeSentence(s);

	let work = $state(entries.map((e) => ({ ...e })));
	let skipped = $state(new Set<string>());
	let allowedDupes = $state(new Set<string>());
	let patch = $state<Record<string, number>>({});
	let intro = $state(true);

	type Rule = 'score' | 'common' | 'rare' | 'all' | 'skip' | 'manual';
	interface RuleOption {
		value: Rule;
		glyph: string;
		label: string;
		note: string;
	}
	const KEEP_ONE: RuleOption[] = [
		{ value: 'score', glyph: '✦', label: 'Highest Auto Mode score', note: 'your points decide' },
		{ value: 'common', glyph: '↑', label: 'More common', note: 'lowest frequency rank' },
		{ value: 'rare', glyph: '↓', label: 'Less common', note: 'highest frequency rank' }
	];
	const OTHERS: RuleOption[] = [
		{
			value: 'all',
			glyph: '⧉',
			label: 'Mine all',
			note: 'Every term is mined, so one sentence can back several cards'
		},
		{
			value: 'skip',
			glyph: '⊘',
			label: 'Skip all',
			note: 'No term in these sentences is mined, except your own picks'
		},
		{
			value: 'manual',
			glyph: '→',
			label: 'Pick each one',
			note: 'Choose the term for each sentence next'
		}
	];
	let rule = $state<Rule>('score');
	let swapFirst = $state(true);

	function groupsOf(list: BatchEntry[]): BatchEntry[][] {
		const map = new Map<string, BatchEntry[]>();
		for (const e of list) {
			const k = skey(e.sentence);
			if (k === '' || skipped.has(e.key) || allowedDupes.has(k)) continue;
			const g = map.get(k);
			if (g) g.push(e);
			else map.set(k, [e]);
		}
		return [...map.values()]
			.filter((g) => g.length >= 2)
			.map((g) =>
				g
					.slice()
					.sort(
						(a, b) => (a.timestamp?.start_secs ?? Infinity) - (b.timestamp?.start_secs ?? Infinity)
					)
			);
	}
	const groups = $derived(groupsOf(work));
	const group = $derived(groups[0]);
	const used = $derived(
		new Set(work.filter((e) => !skipped.has(e.key)).map((e) => skey(e.sentence)))
	);

	const keeperOf = (g: BatchEntry[]) => g.find((e) => e.explicit) ?? g[0];
	const freeAltOf = (e: BatchEntry, usedNow: Set<string>) =>
		e.alternatives.find((a) => !usedNow.has(skey(a.sentence)));

	/** The unused sentence each term moves to in the automatic step. Your own picks never move. */
	const moves = $derived.by(() => {
		const usedNow = new Set(used);
		const out = new Map<string, OccurrenceAlt>();
		for (const g of groups) {
			const keep = keeperOf(g);
			for (const e of g) {
				if (e === keep || e.explicit) continue;
				const alt = freeAltOf(e, usedNow);
				if (alt) {
					out.set(e.key, alt);
					usedNow.add(skey(alt.sentence));
				}
			}
		}
		return out;
	});

	const remaining = $derived(
		swapFirst && moves.size > 0
			? groupsOf(work.map((e) => ({ ...e, sentence: moves.get(e.key)?.sentence ?? e.sentence })))
			: groups
	);

	/** What `rule` leaves of the whole batch. A sentence two of your own picks share is always
	 * left for you to pick, and its terms count as neither mined nor skipped yet. */
	const outcome = $derived.by(() => {
		const selected = work.filter((e) => !skipped.has(e.key)).length;
		let skippedTerms = 0;
		let undecided = 0;
		let byHand = 0;
		for (const g of remaining) {
			const picked = g.filter((e) => e.explicit).length;
			if (rule === 'all') continue;
			if (rule === 'manual' || picked > 1) {
				byHand++;
				undecided += g.length;
			} else {
				skippedTerms += g.length - (rule === 'skip' ? picked : 1);
			}
		}
		return { selected, mined: selected - skippedTerms - undecided, skippedTerms, byHand };
	});

	const COLORS = ['var(--accent)', 'var(--know-young)', 'var(--success)', 'var(--warning)'];
	const colorOf = (i: number) => COLORS[i % COLORS.length];

	// Must match `freqLabel` in TermTable.svelte.
	const freqOf = (e: BatchEntry) => {
		const v = e.term ? harmonic(e.term) : Infinity;
		return v === Infinity ? '？' : String(v);
	};

	const matchIn = (text: string, e: BatchEntry) => {
		const forms = e.term
			? [e.term.surface_form, e.term.full_segment, e.term.lemma_form]
			: [e.surface, e.lemma];
		for (const form of forms) {
			const at = form ? text.indexOf(form) : -1;
			if (at >= 0) return { start: at, end: at + form.length };
		}
		return null;
	};

	const ordered = $derived.by(() => {
		if (!group) return [];
		const text = group[0].sentence;
		return [...group].sort(
			(a, b) => (matchIn(text, a)?.start ?? Infinity) - (matchIn(text, b)?.start ?? Infinity)
		);
	});

	const parts = $derived.by(() => {
		if (!group) return [];
		const text = group[0].sentence;
		const spans: { start: number; end: number; color: string; entry: BatchEntry }[] = [];
		ordered.forEach((e, i) => {
			const m = matchIn(text, e);
			if (m && !spans.some((s) => m.start < s.end && m.end > s.start)) {
				spans.push({ ...m, color: colorOf(i), entry: e });
			}
		});
		spans.sort((a, b) => a.start - b.start);
		const out: { text: string; color: string | null; entry: BatchEntry | null }[] = [];
		let pos = 0;
		for (const s of spans) {
			if (s.start > pos) out.push({ text: text.slice(pos, s.start), color: null, entry: null });
			out.push({ text: text.slice(s.start, s.end), color: s.color, entry: s.entry });
			pos = s.end;
		}
		if (pos < text.length) out.push({ text: text.slice(pos), color: null, entry: null });
		return out;
	});

	// Shift+Hover definition lookup, mirroring SentenceView's segment hovers.
	function hoverTerm(e: MouseEvent, entry: BatchEntry) {
		if (!onlookup) return;
		const el = e.currentTarget as HTMLElement;
		const open = () =>
			onlookup({
				text: entry.lemma,
				label: entry.lemma,
				anchor: el.getBoundingClientRect()
			});
		onhover?.(open);
		if (e.shiftKey) open();
	}

	function reassign(e: BatchEntry, alt: OccurrenceAlt) {
		e.sentence = alt.sentence;
		e.timestamp = alt.timestamp;
		e.surface = alt.surface;
		patch[e.key] = alt.idx;
	}

	function maybeFinish() {
		if (groups.length > 0) return;
		ondone(
			work
				.filter((e) => !skipped.has(e.key))
				.map(({ lemma, key, surface, sentence, timestamp, entryIndex, formatName, scanText }) => ({
					lemma,
					key,
					surface,
					sentence,
					timestamp,
					entryIndex,
					formatName,
					scanText
				})),
			patch
		);
	}

	const rankOf = (e: BatchEntry) => (e.term ? harmonic(e.term) : Infinity);
	const scoreOf = (e: BatchEntry) => {
		const prefs = $settings?.auto_mine;
		if (!e.term || !prefs) return -Infinity;
		const horizon = $knowledge?.horizon ?? DEFAULT_HORIZON;
		return pickPoints(rankOf(e), e.term.part_of_speech, e.term.jlpt_level, horizon, prefs);
	};

	// Best first. A term no frequency list ranks is likely a misparse, so it goes last either way.
	function compare(by: Rule, a: BatchEntry, b: BatchEntry): number {
		const [ra, rb] = [rankOf(a), rankOf(b)];
		if ((ra === Infinity) !== (rb === Infinity)) return ra === Infinity ? 1 : -1;
		if (by === 'score') return scoreOf(b) - scoreOf(a) || ra - rb;
		return by === 'common' ? ra - rb : rb - ra;
	}

	/** Applies `rule` to every shared sentence. Your own sentence picks are kept; a sentence two
	 * of them share is left to pick by hand. */
	function resolve() {
		intro = false;
		if (swapFirst) {
			// `moves` is derived from `work`, which each reassign changes.
			const planned = new Map(moves);
			for (const e of work) {
				const alt = planned.get(e.key);
				if (alt) reassign(e, alt);
			}
		}
		if (rule === 'manual') return maybeFinish();
		const skip = new Set(skipped);
		const dupes = new Set(allowedDupes);
		for (const g of groups) {
			if (rule === 'all') {
				dupes.add(skey(g[0].sentence));
				continue;
			}
			const picked = g.filter((e) => e.explicit);
			if (picked.length > 1) continue;
			const keep =
				picked[0] ?? (rule === 'skip' ? null : [...g].sort((a, b) => compare(rule, a, b))[0]);
			for (const e of g) if (e !== keep) skip.add(e.key);
		}
		skipped = skip;
		allowedDupes = dupes;
		maybeFinish();
	}

	function mineAll() {
		allowedDupes = new Set(allowedDupes).add(skey(group[0].sentence));
		maybeFinish();
	}

	function pickTerm(chosen: BatchEntry) {
		const next = new Set(skipped);
		for (const e of group) {
			if (e !== chosen) next.add(e.key);
		}
		skipped = next;
		maybeFinish();
	}
</script>

<!-- Not dismissible: a stray backdrop click must not discard the batch. -->
<Modal title="Sentence conflicts" width="min(460px, 92%)" dismissible={false} onclose={oncancel}>
	{#if intro && group}
		<div class="summary">
			<span class="big">{groups.length}</span>
			<span>
				shared sentence{groups.length === 1 ? '' : 's'}
				<span class="muted">in a batch of {outcome.selected} terms</span>
			</span>
		</div>

		<div class="auto" class:off={!swapFirst || moves.size === 0}>
			{#if moves.size > 0}
				<label class="auto-row">
					<input type="checkbox" bind:checked={swapFirst} />
					<span
						>⇄ Move {moves.size} term{moves.size === 1 ? '' : 's'} to unused sentences first</span
					>
				</label>
			{:else}
				<span class="auto-row">⇄ No term has an unused sentence to move to</span>
			{/if}
			<span class="auto-note">Your own sentence picks are never moved or dropped.</span>
		</div>

		<div class="decide" role="radiogroup" aria-label="For sentences still shared">
			<p class="decide-title">What happens to the {remaining.length} still shared?</p>
			<div class="group">
				<span class="group-label">Keep one term per sentence</span>
				{#each KEEP_ONE as o (o.value)}
					<label class="opt compact" class:selected={rule === o.value}>
						<input type="radio" value={o.value} bind:group={rule} />
						<span class="glyph" aria-hidden="true">{o.glyph}</span>
						<span class="label">{o.label}</span>
						<span class="note">{o.note}</span>
					</label>
				{/each}
			</div>
			<div class="group">
				{#each OTHERS as o (o.value)}
					<label class="opt" class:selected={rule === o.value}>
						<input type="radio" value={o.value} bind:group={rule} />
						<span class="glyph" aria-hidden="true">{o.glyph}</span>
						<span class="label">{o.label}</span>
						<span class="note">{o.note}</span>
					</label>
				{/each}
			</div>
		</div>
	{:else if group}
		<p class="count">
			{groups.length} conflict{groups.length === 1 ? '' : 's'} remaining — click the term this sentence
			should mine:
		</p>
		<blockquote class="sentence" lang="ja">
			<!-- svelte-ignore a11y_no_static_element_interactions -- Shift+Hover
			     lookup is a mouse affordance; the pick buttons stay keyboard-usable. -->
			{#each parts as p, i (i)}
				{#if p.color && p.entry}<span
						class="hl"
						style:color={p.color}
						onmouseenter={(e) => hoverTerm(e, p.entry!)}
						onmouseleave={() => onhover?.(null)}>{p.text}</span
					>{:else}{p.text}{/if}
			{/each}
		</blockquote>
		<div class="choices">
			{#each ordered as e, i (e.key)}
				<button
					class="pick"
					style:color={colorOf(i)}
					style:border-color={colorOf(i)}
					lang="ja"
					title={`Mine 「${e.lemma}」 from this sentence`}
					onclick={() => pickTerm(e)}
					onmouseenter={(ev) => hoverTerm(ev, e)}
					onmouseleave={() => onhover?.(null)}
				>
					{e.lemma}
					<span class="freq">{freqOf(e)}</span>
				</button>
			{/each}
		</div>
	{/if}
	{#snippet footer()}
		{#if intro && group}
			{@const { selected, mined, skippedTerms, byHand } = outcome}
			{@const sentences = (n: number) => `${n} sentence${n === 1 ? '' : 's'}`}
			<p class="hint outcome">
				{#if rule === 'manual'}
					Mines the {mined} terms outside conflicts; you pick for {sentences(byHand)}.
				{:else}
					Mines {mined} of the {selected} terms and skips {skippedTerms}.
					{#if byHand > 0}You pick for {sentences(byHand)} two of your picks share.{/if}
				{/if}
			</p>
			<footer>
				<button class="right" onclick={oncancel}>Cancel</button>
				<button class="primary" onclick={resolve}
					>{rule === 'manual'
						? `Review ${byHand} conflict${byHand === 1 ? '' : 's'}`
						: 'Resolve'}</button
				>
			</footer>
		{:else if group}
			<p class="hint">Unpicked terms are skipped for this batch.</p>
			<footer>
				<button onclick={mineAll}
					>{group.length === 2 ? 'Mine both' : `Mine all ${group.length}`}</button
				>
				<button class="right" onclick={oncancel}>Cancel batch</button>
			</footer>
		{/if}
	{/snippet}
</Modal>

<style>
	.count {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.9rem;
	}
	.count {
		color: var(--text-muted);
	}
	.summary {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.big {
		font-size: 1.6rem;
		font-weight: 700;
		line-height: 1;
		color: var(--accent);
		font-variant-numeric: tabular-nums;
	}
	.muted {
		margin-left: 0.25rem;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.auto {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		margin: 0 1rem;
		padding: 0.45rem 0.65rem;
		font-size: 0.85rem;
		background: color-mix(in srgb, var(--success) 8%, transparent);
		border-left: 3px solid var(--success);
		border-radius: var(--radius);
	}
	.auto.off {
		background: var(--bg-raised);
		border-left-color: var(--border);
	}
	.auto-row {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.auto-note {
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.decide {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		padding: 0 1rem;
	}
	.decide-title {
		margin: 0.2rem 0 0.3rem;
		font-size: 0.9rem;
		font-weight: 600;
	}
	.group {
		display: flex;
		flex-direction: column;
		margin-bottom: 0.3rem;
		padding: 0.25rem;
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.group-label {
		padding: 0.1rem 0.45rem 0.2rem;
		font-size: 0.7rem;
		letter-spacing: 0.03em;
		text-transform: uppercase;
		color: var(--text-muted);
	}
	/* The whole row is the radio's label. */
	.opt {
		display: grid;
		grid-template-columns: auto 1.2rem auto 1fr;
		column-gap: 0.4rem;
		align-items: baseline;
		padding: 0.35rem 0.45rem;
		font-size: 0.88rem;
		border-left: 3px solid transparent;
		border-radius: var(--radius);
		cursor: pointer;
	}
	.opt input {
		margin: 0;
		align-self: center;
		accent-color: var(--accent);
	}
	.opt:hover {
		background: var(--bg-hover);
	}
	.opt:has(input:focus-visible) {
		outline: 2px solid var(--accent);
	}
	.opt.selected {
		background: color-mix(in srgb, var(--accent) 12%, transparent);
		border-left-color: var(--accent);
	}
	.glyph {
		text-align: center;
		color: var(--text-muted);
	}
	.opt.selected .glyph,
	.opt.selected .label {
		color: var(--accent);
	}
	.label {
		font-weight: 600;
	}
	.note {
		font-size: 0.78rem;
		color: var(--text-muted);
	}
	.opt:not(.compact) .note {
		grid-column: 3 / -1;
	}
	.compact .note {
		justify-self: end;
	}
	.outcome {
		color: var(--text);
	}
	.sentence {
		margin: 0 1rem;
		padding: 0.5rem 0.75rem;
		background: var(--bg-raised);
		border-left: 3px solid var(--accent);
		border-radius: var(--radius);
		font-size: 1.05rem;
	}
	.hl {
		font-weight: 600;
	}
	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.pick {
		cursor: pointer;
		padding: 0.3rem 0.8rem;
		font-size: 1.15rem;
		background: var(--bg-raised);
		border: 1px solid;
		border-radius: var(--radius);
	}
	.pick:hover {
		background: var(--bg-hover);
	}
	.pick .freq {
		margin-left: 0.35rem;
		font-size: 0.75rem;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.hint {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	footer {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	footer .right {
		margin-left: auto;
	}
</style>
