<script lang="ts">
	// App shell: top bar over the main region — the landing screen (recents,
	// open actions) before a file loads, the term table + controls after.
	import { onMount } from 'svelte';
	import { comprehensionColor } from '$lib/comprehension';
	import {
		hydrate,
		openAndProcessFile,
		openRecentFile,
		openAsbplayerModal,
		asbContext,
		playerStatus,
		languageToolsStatus,
		overlay,
		fileResult,
		visibleTerms,
		recentFiles,
		dragHovering,
		lastError,
		notice,
		ankiFilterActive,
		refreshTerms,
		refreshMinedState,
		settings
	} from '$lib/stores';
	import TopBar from '$lib/components/TopBar.svelte';
	import TermTable from '$lib/components/TermTable.svelte';
	import TableControls from '$lib/components/TableControls.svelte';
	import IgnoreListModal from '$lib/components/IgnoreListModal.svelte';
	import WebsocketSettingsModal from '$lib/components/WebsocketSettingsModal.svelte';
	import AppearanceModal from '$lib/components/AppearanceModal.svelte';
	import AboutModal from '$lib/components/AboutModal.svelte';
	import AnkiSettingsModal from '$lib/components/AnkiSettingsModal.svelte';
	import FrequencyWeightsModal from '$lib/components/FrequencyWeightsModal.svelte';
	import PosFiltersModal from '$lib/components/PosFiltersModal.svelte';
	import SetupBanner from '$lib/components/SetupBanner.svelte';
	import SetupChecklistModal from '$lib/components/SetupChecklistModal.svelte';
	import FrequencyAnalyzerModal from '$lib/components/FrequencyAnalyzerModal.svelte';
	import AsbplayerModal from '$lib/components/AsbplayerModal.svelte';
	import TextFiltersModal from '$lib/components/TextFiltersModal.svelte';
	import KnowledgeSummary from '$lib/components/KnowledgeSummary.svelte';

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
	const followOn = $derived(
		($settings?.asbplayer_follow_new_media ?? false) ||
			($settings?.asbplayer_follow_active_tab ?? false)
	);
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

<!-- Focus refresh catches cards mined outside Yomine (issue #3). -->
<svelte:window onkeydown={onKeydown} onfocus={() => void refreshMinedState()} />

<div class="app-shell">
	<TopBar />
	<SetupBanner />

	<main class="app-main">
		{#if toolsError}
			<p class="error">Failed to load language tools: {toolsError}</p>
		{:else if $fileResult}
			{@const pct = $fileResult.file_comprehension * 100}
			{@const total = $fileResult.total_terms}
			{@const known = total - $fileResult.terms.length}
			<div class="header-row">
				<div class="header-left">
					<div class="title-row">
						<h2 class="title">{$fileResult.source_file.title}</h2>
						{#if followOn}
							{#if $asbContext.has_active_tab && !$asbContext.active_has_subtitles}
								<span
									class="tab-chip warn"
									title="Follow can't switch until subtitles are loaded on the active video in asbplayer"
									>● no subtitles on active video</span
								>
							{:else if $asbContext.loaded_from_asbplayer && $asbContext.loaded_is_active}
								<span
									class="tab-chip ok"
									title="This is asbplayer's active tab — mining captures media from it"
									>● active tab</span
								>
							{:else if $asbContext.loaded_from_asbplayer && $asbContext.has_active_tab}
								<span
									class="tab-chip warn"
									title="asbplayer's active tab is a different video — mined media would come from the wrong one"
									>● not the active tab</span
								>
							{/if}
						{/if}
					</div>
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
				</div>
				<KnowledgeSummary />
			</div>
			<TableControls />
			<!-- The one scroll region in the file view: title/coverage/controls above
			     stay put, the sticky column header sticks to this container's top. -->
			<div class="table-scroll">
				<TermTable terms={$visibleTerms} sentences={$fileResult.sentences} />
			</div>
		{:else}
			<div class="landing">
				<h1 class="landing-title">No File Loaded</h1>
				<p class="landing-jp">ファイルがまだ読み込まれていません</p>
				<p class="landing-hint">ℹ You can drag and drop a file at any time to load it.</p>
				<!-- While the language tools load, the landing renders behind the
				     $overlay popup (same loading surface as everywhere else). -->
				<div class="landing-actions">
					<button class="landing-open" disabled={!toolsReady} onclick={openAndProcessFile}
						>Open New File</button
					>
					{#if $playerStatus.ws_clients > 0}
						<!-- Only offered while asbplayer is actually connected (issue #105). -->
						<button class="landing-open asb" disabled={!toolsReady} onclick={openAsbplayerModal}
							>▶ Load from asbplayer</button
						>
					{/if}
				</div>

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
	<AsbplayerModal />
	<WebsocketSettingsModal />
	<AppearanceModal />
	<AboutModal />
	<AnkiSettingsModal />
	<FrequencyWeightsModal />
	<PosFiltersModal />
	<TextFiltersModal />
	<SetupChecklistModal />
	<FrequencyAnalyzerModal />

	{#if $lastError}
		<div class="error-banner" role="alert">
			<strong>{$lastError.title}</strong>
			<span>{$lastError.message}</span>
			{#if $lastError.detail}<span class="detail">{$lastError.detail}</span>{/if}
			<button onclick={() => lastError.set(null)} aria-label="Dismiss">✕</button>
		</div>
	{/if}

	{#if $notice}
		<div class="notice" role="status">{$notice}</div>
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
	.header-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}
	.header-left {
		min-width: 0;
	}
	.title-row {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
	}
	.title {
		margin: 0 0 0.25rem;
	}
	.tab-chip {
		padding: 0.05rem 0.4rem;
		font-size: 0.72rem;
		border-radius: var(--radius);
		white-space: nowrap;
		cursor: help;
	}
	.tab-chip.ok {
		color: var(--success);
		background: color-mix(in srgb, var(--success) 10%, transparent);
		border: 1px solid color-mix(in srgb, var(--success) 35%, transparent);
	}
	.tab-chip.warn {
		color: var(--warning);
		background: color-mix(in srgb, var(--warning) 10%, transparent);
		border: 1px solid color-mix(in srgb, var(--warning) 35%, transparent);
	}
	.comprehension {
		margin: 0 0 0.15rem;
		font-size: 13px;
		font-weight: 600;
	}
	.counts {
		margin: 0 0 1rem;
		font-size: 12px;
		color: var(--text-muted);
	}
	.table-scroll {
		flex: 1 1 auto;
		min-height: 0;
		overflow-y: auto;
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
		color: var(--accent);
	}
	.landing-jp {
		margin: 0.25rem 0 0;
		font-size: 1.125rem;
		color: var(--know-young);
	}
	.landing-hint {
		margin: 0.25rem 0 0;
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.landing-actions {
		display: flex;
		gap: 0.6rem;
		margin-top: 1.25rem;
	}
	.landing-open.asb {
		color: var(--success);
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
		color: var(--accent);
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
	.error {
		color: var(--danger);
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
		background: var(--bg-raised);
		border: 1px solid var(--danger);
		border-radius: var(--radius);
		color: var(--text);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
		/* Above modal backdrops (z 50): errors from modal actions must stay visible. */
		z-index: 60;
	}
	.error-banner strong {
		color: var(--danger);
	}
	/* Transient toast (e.g. follow mode swapped in a new asbplayer video). */
	.notice {
		position: fixed;
		top: 3.2rem;
		left: 50%;
		transform: translateX(-50%);
		max-width: 80vw;
		padding: 0.45rem 0.9rem;
		background: var(--bg-raised);
		border: 1px solid var(--success);
		border-radius: var(--radius);
		color: var(--text);
		font-size: 0.85rem;
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
		/* Above modal backdrops (z 50): follow-mode loads can land mid-modal. */
		z-index: 60;
	}
	.error-banner .detail {
		color: var(--text-muted);
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
		background: color-mix(in srgb, var(--bg-deep) 75%, transparent);
		color: var(--text);
		font-size: 1.1rem;
	}
	.drop-overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		/* Don't intercept the native OS drop. */
		pointer-events: none;
		z-index: 20;
	}
	.drop-card {
		padding: 2rem 3rem;
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--accent);
		background: var(--bg-raised);
		border: 2px dashed var(--accent);
		border-radius: var(--radius);
	}
</style>
