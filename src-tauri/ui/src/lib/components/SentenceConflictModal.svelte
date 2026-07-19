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
		term: Term;
		key: string;
		/** The occurrence text the table highlighted (cloze/bold). */
		surface: string;
		sentence: string;
		timestamp: TimeStampDto | null;
		/** Yomitan entry chosen via the popover's Queue (default first). */
		entryIndex?: number;
		/** Yomitan card format chosen via the popover's Queue (default first). */
		formatName?: string;
		/** The user explicitly navigated to this occurrence. */
		explicit: boolean;
		/** Occurrences with a different sentence than the chosen one. */
		alternatives: OccurrenceAlt[];
	}
</script>

<script lang="ts">
	import { harmonic } from '$lib/table';
	import { normalizeSentence, type QueueItem } from '$lib/stores';

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

	const groups = $derived.by(() => {
		const map = new Map<string, BatchEntry[]>();
		for (const e of work) {
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
	});
	const group = $derived(groups[0]);
	const used = $derived(new Set(work.filter((e) => !skipped.has(e.key)).map((e) => skey(e.sentence))));

	const keeperOf = (g: BatchEntry[]) => g.find((e) => e.explicit) ?? g[0];
	const freeAltOf = (e: BatchEntry, usedNow: Set<string>) =>
		e.alternatives.find((a) => !usedNow.has(skey(a.sentence)));

	const autoResolvable = $derived(
		groups.reduce((n, g) => {
			const keep = keeperOf(g);
			return n + g.filter((e) => e !== keep && !e.explicit && freeAltOf(e, used)).length;
		}, 0)
	);

	const COLORS = ['var(--cyan)', 'var(--orange)', 'var(--green)', 'var(--yellow)'];
	const colorOf = (i: number) => COLORS[i % COLORS.length];

	// Must match `freqLabel` in TermTable.svelte.
	const freqOf = (e: BatchEntry) => {
		const v = harmonic(e.term);
		return v === Infinity ? '？' : String(v);
	};

	const matchIn = (text: string, e: BatchEntry) => {
		for (const form of [e.term.surface_form, e.term.full_segment, e.term.lemma_form]) {
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
				text: entry.term.lemma_form,
				label: entry.term.lemma_form,
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
				.map(({ term, surface, sentence, timestamp, entryIndex, formatName }) => ({
					term,
					surface,
					sentence,
					timestamp,
					entryIndex,
					formatName
				})),
			patch
		);
	}

	function autoResolve() {
		intro = false;
		const usedNow = new Set(used);
		for (const g of groups.map((g) => [...g])) {
			const keep = keeperOf(g);
			for (const e of g) {
				if (e === keep || e.explicit) continue;
				const alt = freeAltOf(e, usedNow);
				if (alt) {
					reassign(e, alt);
					usedNow.add(skey(alt.sentence));
				}
			}
		}
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

<svelte:window onkeydown={(e) => e.key === 'Escape' && oncancel()} />

<!-- Only direct backdrop clicks cancel; clicks inside the dialog must still
     bubble to window so the definition popover's close-on-click works. -->
<div
	class="backdrop"
	role="button"
	tabindex="-1"
	onclick={(e) => e.target === e.currentTarget && oncancel()}
	onkeydown={(e) => e.key === 'Escape' && oncancel()}
>
	<div class="dialog" role="dialog" aria-modal="true" aria-label="Sentence conflicts" tabindex="-1">
		<header>
			<h2>Sentence conflicts</h2>
			<button class="close" aria-label="Close" onclick={oncancel}>✕</button>
		</header>

		{#if intro && group}
			<p class="body">
				{groups.length} sentence{groups.length === 1 ? ' is' : 's are'} shared by more than one
				selected term.
				{#if autoResolvable > 0}
					{autoResolvable} term{autoResolvable === 1 ? '' : 's'} can switch to an unused sentence
					automatically; your own sentence picks are never changed.
				{:else}
					None of them have an unused sentence to switch to, so each conflict needs a pick.
				{/if}
			</p>
			<footer>
				{#if autoResolvable > 0}
					<button onclick={autoResolve}>Auto-swap sentences</button>
				{/if}
				<button onclick={() => (intro = false)}>Pick manually</button>
				<button class="right" onclick={oncancel}>Cancel</button>
			</footer>
		{:else if group}
			<p class="count">
				{groups.length} conflict{groups.length === 1 ? '' : 's'} remaining — click the term this
				sentence should mine:
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
						title={`Mine 「${e.term.lemma_form}」 from this sentence`}
						onclick={() => pickTerm(e)}
						onmouseenter={(ev) => hoverTerm(ev, e)}
						onmouseleave={() => onhover?.(null)}
					>
						{e.term.lemma_form}
						<span class="freq">{freqOf(e)}</span>
					</button>
				{/each}
			</div>
			<p class="hint">Unpicked terms are skipped for this batch.</p>
			<footer>
				<button onclick={mineAll}>{group.length === 2 ? 'Mine both' : `Mine all ${group.length}`}</button>
				<button class="right" onclick={oncancel}>Cancel batch</button>
			</footer>
		{/if}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-darker) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(460px, 92%);
		padding-bottom: 0.75rem;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--border);
	}
	header h2 {
		margin: 0;
		font-size: 1.05rem;
		color: var(--cyan);
	}
	.close {
		padding: 0.1rem 0.4rem;
	}
	.body,
	.count {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.9rem;
	}
	.count {
		color: var(--comment);
	}
	.sentence {
		margin: 0 1rem;
		padding: 0.5rem 0.75rem;
		background: var(--bg-light);
		border-left: 3px solid var(--cyan);
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
		background: var(--bg-light);
		border: 1px solid;
		border-radius: var(--radius);
	}
	.pick:hover {
		background: var(--bg-lighter);
	}
	.pick .freq {
		margin-left: 0.35rem;
		font-size: 0.75rem;
		color: var(--comment);
		font-variant-numeric: tabular-nums;
	}
	.hint {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.8rem;
		color: var(--comment);
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
