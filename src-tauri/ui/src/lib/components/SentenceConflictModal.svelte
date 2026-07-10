<script module lang="ts">
	import type { Term, TimeStampDto } from '$lib/ipc';

	export interface OccurrenceAlt {
		/** Index into the row's occurrence list (syncs `occIdx` on reassign). */
		idx: number;
		sentence: string;
		timestamp: TimeStampDto | null;
	}

	/** One queued term with what's needed to resolve sentence conflicts. */
	export interface BatchEntry {
		term: Term;
		key: string;
		sentence: string;
		timestamp: TimeStampDto | null;
		/** The user explicitly navigated to this occurrence. */
		explicit: boolean;
		/** Occurrences with a different sentence than the chosen one. */
		alternatives: OccurrenceAlt[];
	}
</script>

<script lang="ts">
	import { normalizeSentence, type QueueItem } from '$lib/stores';

	let {
		entries,
		ondone,
		oncancel
	}: {
		entries: BatchEntry[];
		ondone: (items: QueueItem[], occIdxPatch: Record<string, number>) => void;
		oncancel: () => void;
	} = $props();

	const skey = (s: string) => normalizeSentence(s);

	let work = $state(entries.map((e) => ({ ...e })));
	let skipped = $state(new Set<string>());
	/** Sentence keys the user chose to mine as duplicates. */
	let allowed = $state(new Set<string>());
	let patch = $state<Record<string, number>>({});
	let intro = $state(true);

	// Unresolved conflict groups, each in timestamp order.
	const groups = $derived.by(() => {
		const map = new Map<string, BatchEntry[]>();
		for (const e of work) {
			const k = skey(e.sentence);
			if (k === '' || skipped.has(e.key) || allowed.has(k)) continue;
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

	// The user's explicit pick outranks defaults; ties go to the earliest cue.
	const keeperOf = (g: BatchEntry[]) => g.find((e) => e.explicit) ?? g[0];
	const freeAltOf = (e: BatchEntry, usedNow: Set<string>) =>
		e.alternatives.find((a) => !usedNow.has(skey(a.sentence)));

	const autoResolvable = $derived(
		groups.reduce((n, g) => {
			const keep = keeperOf(g);
			return n + g.filter((e) => e !== keep && !e.explicit && freeAltOf(e, used)).length;
		}, 0)
	);
	const groupHasAlt = $derived(
		group !== undefined && group.some((e) => e !== keeperOf(group) && freeAltOf(e, used))
	);

	function reassign(e: BatchEntry, alt: OccurrenceAlt) {
		e.sentence = alt.sentence;
		e.timestamp = alt.timestamp;
		patch[e.key] = alt.idx;
	}

	function maybeFinish() {
		if (groups.length > 0) return;
		ondone(
			work
				.filter((e) => !skipped.has(e.key))
				.map(({ term, sentence, timestamp }) => ({ term, sentence, timestamp })),
			patch
		);
	}

	/** Move every non-explicit duplicate with an unused sentence off it. */
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

	function mineAnyway() {
		allowed = new Set(allowed).add(skey(group[0].sentence));
		maybeFinish();
	}

	/** Keeper keeps; others move where an unused sentence exists, the rest
	 * stay shared. */
	function useOthers() {
		const g = [...group];
		const keep = keeperOf(g);
		const usedNow = new Set(used);
		let leftover = false;
		for (const e of g) {
			if (e === keep) continue;
			const alt = freeAltOf(e, usedNow);
			if (alt) {
				reassign(e, alt);
				usedNow.add(skey(alt.sentence));
			} else {
				leftover = true;
			}
		}
		if (leftover) allowed = new Set(allowed).add(skey(keep.sentence));
		maybeFinish();
	}

	function skipDupes() {
		const g = [...group];
		const keep = keeperOf(g);
		const next = new Set(skipped);
		for (const e of g) if (e !== keep) next.add(e.key);
		skipped = next;
		maybeFinish();
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && oncancel()} />

<div
	class="backdrop"
	role="button"
	tabindex="-1"
	onclick={oncancel}
	onkeydown={(e) => e.key === 'Escape' && oncancel()}
>
	<!-- Stop backdrop clicks inside the dialog from closing it. -->
	<div
		class="dialog"
		role="dialog"
		aria-modal="true"
		aria-label="Sentence conflicts"
		tabindex="-1"
		onclick={(e) => e.stopPropagation()}
	>
		<header>
			<h2>Sentence conflicts</h2>
			<button class="close" aria-label="Close" onclick={oncancel}>✕</button>
		</header>

		{#if intro && autoResolvable > 0}
			<p class="body">
				{groups.length} sentence{groups.length === 1 ? ' is' : 's are'} shared by more than one
				selected term. {autoResolvable} term{autoResolvable === 1 ? '' : 's'} can switch to an
				unused sentence automatically; your own sentence picks are never changed.
			</p>
			<footer>
				<button onclick={autoResolve}>Auto-resolve</button>
				<button onclick={() => (intro = false)}>Resolve one by one</button>
				<button class="right" onclick={oncancel}>Cancel</button>
			</footer>
		{:else if group}
			<p class="count">
				{groups.length} conflict{groups.length === 1 ? '' : 's'} remaining — these terms would all
				mine:
			</p>
			<blockquote class="sentence" lang="ja">{group[0].sentence}</blockquote>
			<ul class="terms">
				{#each group as e (e.key)}
					<li>
						<span class="lemma" lang="ja">{e.term.lemma_form}</span>
						{#if e === keeperOf(group)}
							<span class="tag">keeps this sentence</span>
						{:else if e.explicit}
							<span class="tag">your pick</span>
						{/if}
					</li>
				{/each}
			</ul>
			<footer>
				<button onclick={mineAnyway}>Mine anyway</button>
				{#if groupHasAlt}
					<button onclick={useOthers}>Use another sentence</button>
				{/if}
				<button onclick={skipDupes}>Skip duplicates</button>
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
	.terms {
		margin: 0;
		padding: 0 1rem 0 2.2rem;
	}
	.terms li {
		padding: 0.1rem 0;
	}
	.lemma {
		font-size: 1.1rem;
		color: var(--red);
	}
	.tag {
		margin-left: 0.5rem;
		font-size: 0.75rem;
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
