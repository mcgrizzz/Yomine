<script lang="ts">
	import Modal from './Modal.svelte';
	import { recentFiles, recentFilesModalOpen, openRecentFile } from '$lib/stores';
	import { fileIcon, filename, formatTermCount, formatFileSize, formatLastOpened } from '$lib/recents';

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
	{#if $recentFiles.length === 0}
		<p class="empty">No recent files.</p>
	{:else}
		<ul class="list">
			{#each $recentFiles as entry (entry.file_path)}
				<li>
					<button class="recent" title={entry.file_path} onclick={() => open(entry.file_path)}>
						<span class="recent-name"
							>{fileIcon(entry.file_path)}
							{entry.title.trim() || filename(entry.file_path)}</span
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
