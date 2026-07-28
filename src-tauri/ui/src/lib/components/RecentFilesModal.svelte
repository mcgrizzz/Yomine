<script lang="ts">
	import { recentFiles, recentFilesModalOpen, openRecentFile } from '$lib/stores';
	import { fileIcon, filename, formatTermCount, formatFileSize, formatLastOpened } from '$lib/recents';

	function open(path: string) {
		recentFilesModalOpen.set(false);
		void openRecentFile(path);
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window
	onkeydown={(e) => $recentFilesModalOpen && e.key === 'Escape' && recentFilesModalOpen.set(false)}
/>

{#if $recentFilesModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => recentFilesModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && recentFilesModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Recent files"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Recent Files ({$recentFiles.length})</h2>
				<button class="close" aria-label="Close" onclick={() => recentFilesModalOpen.set(false)}
					>✕</button
				>
			</header>

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
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(620px, 92%);
		max-height: 82%;
		padding-bottom: 0.75rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem 0;
	}
	h2 {
		margin: 0;
		font-size: 1rem;
	}
	.close {
		padding: 0.1rem 0.4rem;
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
		color: var(--know-young);
	}
</style>
