<script lang="ts">
	import { autoLedger } from '$lib/stores';
</script>

<ul class="ledger">
	{#each $autoLedger as e, i (i)}
		<li class:undone={e.undone}>
			<span class="mark" aria-hidden="true">{e.undone ? '↶' : e.created > 0 ? '✓' : '–'}</span>
			<span class="title" title={e.title}>{e.title}</span>
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
