<script lang="ts">
	import Modal from './Modal.svelte';
	import type { MiningMode } from '$lib/ipc';
	import { recentFiles, recentFilesModalOpen, openRecentFile } from '$lib/stores';
	import {
		fileIcon,
		filename,
		formatTermCount,
		formatFileSize,
		formatLastOpened,
		inMode,
		modeLabel,
		recentKey
	} from '$lib/recents';

	let filter = $state<MiningMode | 'all'>('all');
	const shown = $derived.by(() => {
		const mode = filter;
		return mode === 'all' ? $recentFiles : $recentFiles.filter((entry) => inMode(entry, mode));
	});

	function open(path: string) {
		recentFilesModalOpen.set(false);
		void openRecentFile(path);
	}
</script>

<Modal
	open={$recentFilesModalOpen}
	title="Recent Files ({$recentFiles.length})"
	width="min(620px, 92%)"
	onclose={() => recentFilesModalOpen.set(false)}
>
	<div class="filters" role="group" aria-label="Show">
		{#each [['all', 'All'], ['local', 'Local'], ['asbplayer', 'asbplayer']] as [value, label] (value)}
			<button
				class="filter"
				aria-pressed={filter === value}
				onclick={() => (filter = value as MiningMode | 'all')}>{label}</button
			>
		{/each}
	</div>
	{#if shown.length === 0}
		<p class="empty">No recent files.</p>
	{:else}
		<ul class="list">
			{#each shown as entry (recentKey(entry))}
				<li>
					<button class="recent" title={entry.file_path} onclick={() => open(entry.file_path)}>
						<span class="recent-name"
							>{fileIcon(entry.file_path)}
							{entry.title.trim() || filename(entry.file_path)}
							{#if entry.mode}<span class="mode-badge">{modeLabel(entry.mode)}</span>{/if}</span
						>
						{#if entry.subtitle}
							<span class="recent-file">{entry.subtitle}</span>
						{/if}
						{#if entry.title.trim() && entry.title !== filename(entry.file_path)}
							<span class="recent-file">{filename(entry.file_path)}</span>
						{/if}
						<span class="recent-meta">
							<span class="recent-terms">{formatTermCount(entry.term_count)}</span>
							{#if entry.creator}<span class="recent-creator">📷 {entry.creator}</span>{/if}
							<span>{formatLastOpened(entry.last_opened)}</span>
							<span>{formatFileSize(entry.file_size)}</span>
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</Modal>

<style>
	.filters {
		display: flex;
		gap: 0.3rem;
		padding: 0 1rem 0.6rem;
	}
	.filter {
		padding: 0.15rem 0.6rem;
		font-size: 0.75rem;
		border-radius: var(--radius);
	}
	.filter[aria-pressed='true'] {
		border-color: var(--accent);
		color: var(--accent);
	}
	.mode-badge {
		margin-left: 0.4rem;
		padding: 0 0.35rem;
		font-size: 0.65rem;
		color: var(--text-muted);
		border: 1px solid var(--border);
		border-radius: var(--radius);
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
		padding: 0 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		overflow-y: auto;
	}
	.recent {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		width: 100%;
		padding: 0.5rem 0.7rem;
		text-align: left;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.recent:hover {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.recent-name {
		font-size: 0.9rem;
		color: var(--text);
	}
	.recent-file {
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.recent-meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.6rem;
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.recent-terms {
		color: var(--info);
	}
	.recent-creator {
		color: var(--text-muted);
	}
</style>
