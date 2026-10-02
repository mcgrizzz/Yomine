<script lang="ts">
	import Modal from './Modal.svelte';
	import RecentCard from './RecentCard.svelte';
	import type { MiningMode, RecentFileEntry } from '$lib/ipc';
	import { recentFiles, recentFilesModalOpen, openRecentFile } from '$lib/stores';
	import { filename, recentKey } from '$lib/recents';

	type Filter = MiningMode | 'all';

	let filter = $state<Filter>('all');
	let search = $state('');

	// Books count as Local: they're files on disk, though opening one keeps the mode.
	const inFilter = (entry: RecentFileEntry, f: Filter) =>
		f === 'all' || (entry.mode ?? 'local') === f;
	const filters: [Filter, string][] = [
		['all', 'All'],
		['local', 'Local'],
		['asbplayer', 'asbplayer']
	];
	const counts = $derived(
		Object.fromEntries(
			filters.map(([f]) => [f, $recentFiles.filter((entry) => inFilter(entry, f)).length])
		) as Record<Filter, number>
	);
	const shown = $derived.by(() => {
		const f = filter;
		const query = search.trim().toLowerCase();
		return $recentFiles.filter(
			(entry) =>
				inFilter(entry, f) &&
				(!query ||
					[entry.title, entry.subtitle ?? '', filename(entry.file_path)].some((text) =>
						text.toLowerCase().includes(query)
					))
		);
	});

	function open(entry: RecentFileEntry) {
		recentFilesModalOpen.set(false);
		void openRecentFile(entry);
	}
</script>

<Modal
	open={$recentFilesModalOpen}
	title="Recent Files"
	width="min(620px, 92%)"
	onclose={() => recentFilesModalOpen.set(false)}
>
	<div class="controls">
		<div class="filters" role="radiogroup" aria-label="Show">
			{#each filters as [value, label] (value)}
				<button
					role="radio"
					aria-checked={filter === value}
					class:on={filter === value}
					onclick={() => (filter = value)}
					>{label} <span class="count">{counts[value]}</span></button
				>
			{/each}
		</div>
		<input
			class="search"
			type="search"
			placeholder="Search titles and files"
			aria-label="Search recent files"
			bind:value={search}
		/>
	</div>
	{#if $recentFiles.length === 0}
		<p class="empty">No recent files.</p>
	{:else if shown.length === 0}
		<p class="empty">No matching files.</p>
	{:else}
		<ul class="list">
			{#each shown as entry (recentKey(entry))}
				<li><RecentCard {entry} showBadge={filter === 'all'} onopen={() => open(entry)} /></li>
			{/each}
		</ul>
	{/if}
</Modal>

<style>
	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.6rem;
		padding: 0 1rem 0.6rem;
	}
	.filters {
		display: inline-flex;
		padding: 2px;
		border: 1px solid var(--border);
		border-radius: 999px;
	}
	.filters button {
		padding: 0.15rem 0.7rem;
		font-size: 0.75rem;
		color: var(--text-muted);
		background: transparent;
		border: none;
		border-radius: 999px;
	}
	.filters button.on {
		color: var(--bg);
		background: var(--accent);
	}
	.filters button:not(.on):hover {
		color: var(--text);
	}
	.count {
		opacity: 0.75;
	}
	.search {
		flex: 1 1 10rem;
		min-width: 0;
		font-size: 0.8rem;
	}
	.empty {
		margin: 0;
		padding: 0 1rem;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.list {
		list-style: none;
		margin: 0;
		padding: 0 1rem 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		overflow-y: auto;
		/* Fades over the bottom padding, so a card is only faded while more follow. */
		mask-image: linear-gradient(to bottom, black calc(100% - 1rem), transparent);
	}
</style>
