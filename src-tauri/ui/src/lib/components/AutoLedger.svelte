<script lang="ts">
	import { autoLedger } from '$lib/stores';

	const BOUNDARY = /[\s\-_:.,・|[\]()（）「」]/;

	/** Titles without the text they all share at either end, cut at word boundaries. */
	function distinct(titles: string[]): string[] {
		if (new Set(titles).size < 2) return titles;
		const shortest = Math.min(...titles.map((t) => t.length));
		const same = (at: (t: string) => string) => titles.every((t) => at(t) === at(titles[0]));
		let pre = 0;
		while (pre < shortest && same((t) => t[pre])) pre++;
		let suf = 0;
		while (suf < shortest - pre && same((t) => t[t.length - 1 - suf])) suf++;
		const first = titles[0];
		while (pre > 0 && !BOUNDARY.test(first[pre - 1])) pre--;
		while (suf > 0 && !BOUNDARY.test(first[first.length - suf])) suf--;
		return titles.map((t) => {
			const part = t.slice(pre, t.length - suf).replace(/^[\s\-_:.|]+|[\s\-_:.|]+$/g, '');
			return part || t;
		});
	}

	const names = $derived(distinct($autoLedger.map((e) => e.title)));
</script>

<ul class="ledger">
	{#each $autoLedger as e, i (i)}
		<li class:undone={e.undone}>
			<span class="mark" aria-hidden="true">{e.undone ? '↶' : e.created > 0 ? '✓' : '–'}</span>
			<span class="title" title={e.title}>{names[i]}</span>
			<span class="count">
				{e.undone ? 'undone' : e.batchId === null ? 'nothing to mine' : `${e.created} cards`}
			</span>
		</li>
	{:else}
		<li class="empty">Nothing mined yet</li>
	{/each}
</ul>

<style>
	.ledger {
		display: grid;
		gap: 0.2rem;
		margin: 0;
		padding: 0;
		list-style: none;
		font-size: 0.8rem;
	}
	li {
		display: flex;
		align-items: baseline;
		gap: 0.45rem;
		min-width: 0;
	}
	.mark {
		width: 0.9rem;
		flex-shrink: 0;
		color: var(--success);
		text-align: center;
	}
	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.count {
		flex-shrink: 0;
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
	}
	.undone .mark,
	.undone .title {
		color: var(--text-muted);
	}
	.empty {
		color: var(--text-muted);
	}
</style>
