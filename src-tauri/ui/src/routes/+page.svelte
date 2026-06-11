<script lang="ts">
	// App shell (T026): top bar + scrolling main region, matching egui's IA
	// (TopBar over a central term area). The full menu is `TopBar` (T028); the
	// virtualized term table + sentence view replace the placeholder list in
	// T029/T030. The startup round trip (open file → term count) stays working.
	import { onMount } from 'svelte';
	import { comprehensionColor } from '$lib/comprehension';
	import {
		hydrate,
		openAndProcessFile,
		openRecentFile,
		languageToolsStatus,
		overlay,
		fileResult,
		visibleTerms,
		recentFiles,
		dragHovering,
		lastError,
		ankiFilterActive,
		refreshTerms
	} from '$lib/stores';
	import TopBar from '$lib/components/TopBar.svelte';
	import TermTable from '$lib/components/TermTable.svelte';
	import TableControls from '$lib/components/TableControls.svelte';
	import IgnoreListModal from '$lib/components/IgnoreListModal.svelte';
	import WebsocketSettingsModal from '$lib/components/WebsocketSettingsModal.svelte';

	onMount(hydrate);

	// Display helpers mirroring egui's `RecentFileEntry` formatters.
	const filename = (path: string) => path.split(/[\\/]/).pop() ?? path;

	function formatTermCount(n: number | null): string {
		if (n === null) return 'Unknown terms';
		return n === 1 ? '1 term' : `${n} terms`;
	}

	function formatFileSize(bytes: number | null): string {
		if (bytes === null) return 'Unknown';
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	function formatLastOpened(iso: string): string {
		const d = new Date(iso);
		const p = (n: number) => String(n).padStart(2, '0');
		return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
	}

	const toolsReady = $derived($languageToolsStatus === 'ready');
	const toolsError = $derived(
		typeof $languageToolsStatus === 'object' ? $languageToolsStatus.error : null
	);

	// F5 / Ctrl+R refresh terms (and block the webview's page reload).
	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'F5' || ((e.key === 'r' || e.key === 'R') && (e.ctrlKey || e.metaKey))) {
			e.preventDefault();
			refreshTerms();
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app-shell">
	<TopBar />

	<main class="app-main">
		{#if toolsError}
			<p class="error">Failed to load language tools: {toolsError}</p>
		{:else if !toolsReady}
			<p class="muted">Loading language tools…</p>
		{:else if $fileResult}
			{@const pct = $fileResult.file_comprehension * 100}
			{@const total = $fileResult.total_terms}
			{@const known = total - $fileResult.terms.length}
			<h2 class="title">{$fileResult.source_file.title}</h2>
			{#if $ankiFilterActive && $fileResult.sentences.length > 0}
				<p
					class="comprehension"
					style:color={comprehensionColor(pct)}
					title="Overall estimated comprehension across all sentences"
				>
					Comprehension estimate: {pct.toFixed(1)}%
				</p>
			{/if}
			<p class="counts">
				{$visibleTerms.length} shown
				{#if known > 0}
					/ <span
						title={`Ignore list: ${$fileResult.ignored_terms}\nAnki filtered: ${known - $fileResult.ignored_terms}`}
						>{known} known</span
					>
				{/if}
				/ {total} total
			</p>
			<TableControls />
			<TermTable terms={$visibleTerms} sentences={$fileResult.sentences} />
		{:else}
			<div class="landing">
				<h1 class="landing-title">No File Loaded</h1>
				<p class="landing-jp">ファイルがまだ読み込まれていません</p>
				<p class="landing-hint">ℹ You can drag and drop a file at any time to load it.</p>
				<button class="landing-open" onclick={openAndProcessFile}>Open New File</button>

				{#if $recentFiles.length > 0}
					<section class="recents">
						<h2 class="recents-title">Recent Files ({$recentFiles.length})</h2>
						<ul class="recents-list">
							{#each $recentFiles as entry (entry.file_path)}
								<li>
									<button
										class="recent"
										title={entry.file_path}
										onclick={() => openRecentFile(entry.file_path)}
									>
										<span class="recent-name"
											>{entry.title.trim() || filename(entry.file_path)}</span
										>
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
					</section>
				{/if}
			</div>
		{/if}
	</main>

	<IgnoreListModal />
	<WebsocketSettingsModal />

	{#if $lastError}
		<div class="error-banner" role="alert">
			<strong>{$lastError.title}</strong>
			<span>{$lastError.message}</span>
			{#if $lastError.detail}<span class="detail">{$lastError.detail}</span>{/if}
			<button onclick={() => lastError.set(null)} aria-label="Dismiss">✕</button>
		</div>
	{/if}

	{#if $dragHovering}
		<div class="drop-overlay">
			<div class="drop-card">📥&nbsp; Drop to open</div>
		</div>
	{/if}

	{#if $overlay}
		<div class="overlay">{$overlay}</div>
	{/if}
</div>

<style>
	.title {
		margin: 0 0 0.25rem;
	}
	.comprehension {
		margin: 0 0 0.15rem;
		font-size: 13px;
		font-weight: 600;
	}
	.counts {
		margin: 0 0 1rem;
		font-size: 12px;
		color: var(--comment);
	}
	.muted {
		color: var(--comment);
	}
	.landing {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding-top: 5rem;
		/* Fill the (bounded) main region so only the recents list scrolls,
		   never the whole welcome screen. */
		height: 100%;
		min-height: 0;
	}
	.landing-title {
		margin: 0;
		font-size: 2rem;
		font-weight: 700;
		color: var(--cyan);
	}
	.landing-jp {
		margin: 0.25rem 0 0;
		font-size: 1.125rem;
		color: var(--orange);
	}
	.landing-hint {
		margin: 0.25rem 0 0;
		font-size: 0.75rem;
		color: var(--comment);
	}
	.landing-open {
		margin-top: 1.25rem;
	}
	.recents {
		margin-top: 2.5rem;
		width: min(640px, 90%);
		text-align: left;
		/* Grow into the remaining height; the list inside is the scroll region. */
		flex: 1 1 auto;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}
	.recents-title {
		margin: 0 0 0.5rem;
		font-size: 0.85rem;
		font-weight: 600;
		color: var(--cyan);
	}
	.recents-list {
		list-style: none;
		margin: 0;
		padding: 0 0.4rem 0.5rem 0;
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		/* The one scrollable region on the welcome screen. */
		flex: 1 1 auto;
		min-height: 0;
		overflow-y: auto;
	}
	.recent {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		width: 100%;
		padding: 0.5rem 0.7rem;
		text-align: left;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.recent:hover {
		background: var(--bg-lighter);
		border-color: var(--cyan);
	}
	.recent-name {
		font-size: 0.9rem;
		color: var(--fg);
	}
	.recent-file {
		font-size: 0.7rem;
		color: var(--comment);
	}
	.recent-meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.6rem;
		font-size: 0.7rem;
		color: var(--comment);
	}
	.recent-terms {
		color: var(--blue);
	}
	.recent-creator {
		color: var(--orange);
	}
	.error {
		color: var(--red);
	}
	.error-banner {
		position: fixed;
		left: 50%;
		bottom: 1rem;
		transform: translateX(-50%);
		display: flex;
		align-items: center;
		gap: 0.6rem;
		max-width: 90vw;
		padding: 0.6rem 0.9rem;
		background: var(--bg-light);
		border: 1px solid var(--red);
		border-radius: var(--radius);
		color: var(--fg);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
	}
	.error-banner strong {
		color: var(--red);
	}
	.error-banner .detail {
		color: var(--comment);
		font-size: 0.85rem;
	}
	.error-banner button {
		padding: 0.1rem 0.4rem;
	}
	.overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-darker) 75%, transparent);
		color: var(--fg);
		font-size: 1.1rem;
	}
	.drop-overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-darker) 70%, transparent);
		/* Don't intercept the native OS drop. */
		pointer-events: none;
		z-index: 20;
	}
	.drop-card {
		padding: 2rem 3rem;
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--cyan);
		background: var(--bg-light);
		border: 2px dashed var(--cyan);
		border-radius: var(--radius);
	}
</style>
