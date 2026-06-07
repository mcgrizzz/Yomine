<script lang="ts">
	// App shell (T026): top bar + scrolling main region, matching egui's IA
	// (TopBar over a central term area). The full menu lands in T028 and the
	// virtualized term table + sentence view replace the placeholder list in
	// T029/T030. The startup round trip (open file → term count) stays working.
	import { onMount } from 'svelte';
	import {
		hydrate,
		openAndProcessFile,
		openRecentFile,
		languageToolsStatus,
		overlay,
		fileResult,
		recentFiles,
		dragHovering,
		ankiStatus,
		playerStatus,
		lastError
	} from '$lib/stores';
	import TermTable from '$lib/components/TermTable.svelte';

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
	const playerLabel = $derived(
		$playerStatus.mode === 'mpv'
			? 'MPV'
			: $playerStatus.mode === 'asbplayer'
				? 'asbplayer'
				: 'Player'
	);
</script>

<div class="app-shell">
	<header class="topbar">
		<span class="brand">Yomine</span>
		<!-- TODO(T028): full menu — file, anki, websocket, ignore list, freq weights,
		     POS filters, analyzer, setup checklist. -->
		<button onclick={openAndProcessFile} disabled={!toolsReady}>Open file…</button>
		<span class="spacer"></span>
		<span class="status">
			<span class="chip" class:on={$ankiStatus.connected}>Anki</span>
			<span class="chip" class:on={$playerStatus.mode !== 'none'}>{playerLabel}</span>
		</span>
	</header>

	<main class="app-main">
		{#if toolsError}
			<p class="error">Failed to load language tools: {toolsError}</p>
		{:else if !toolsReady}
			<p class="muted">Loading language tools…</p>
		{:else if $fileResult}
			<h2 class="title">{$fileResult.source_file.title}</h2>
			<p class="meta">
				<strong>{$fileResult.terms.length}</strong> minable terms ·
				{$fileResult.sentences.length} sentences ·
				{Math.round($fileResult.file_comprehension * 100)}% comprehension
			</p>
			<TermTable terms={$fileResult.terms} sentences={$fileResult.sentences} />
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
	.topbar {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 1rem;
		background: var(--bg-dark);
		border-bottom: 1px solid var(--border);
	}
	.brand {
		font-weight: 700;
		font-size: 1.05rem;
	}
	.spacer {
		flex: 1;
	}
	.status {
		display: flex;
		gap: 0.5rem;
	}
	.chip {
		font-size: 0.75rem;
		padding: 0.15rem 0.5rem;
		border-radius: 999px;
		background: var(--bg-light);
		color: var(--comment);
		border: 1px solid var(--border);
	}
	.chip.on {
		color: var(--bg-darker);
		background: var(--green);
		border-color: var(--green);
	}
	.title {
		margin: 0 0 0.25rem;
	}
	.meta {
		margin: 0 0 1rem;
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
