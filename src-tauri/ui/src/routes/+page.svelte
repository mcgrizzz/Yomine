<script lang="ts">
	// App shell (T026): top bar + scrolling main region, matching egui's IA
	// (TopBar over a central term area). The full menu lands in T028 and the
	// virtualized term table + sentence view replace the placeholder list in
	// T029/T030. The startup round trip (open file → term count) stays working.
	import { onMount } from 'svelte';
	import {
		hydrate,
		openAndProcessFile,
		languageToolsStatus,
		overlay,
		fileResult,
		ankiStatus,
		playerStatus
	} from '$lib/stores';
	import TermTable from '$lib/components/TermTable.svelte';

	onMount(hydrate);

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
			<TermTable terms={$fileResult.terms} />
		{:else}
			<p class="muted">Open a subtitle or text file to start mining.</p>
		{/if}
	</main>

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
	.error {
		color: var(--red);
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
</style>
