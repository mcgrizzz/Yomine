<script lang="ts">
	// Top bar (T028): mirrors egui's `top_bar.rs` — theme/font toggles, the
	// File / Settings / Tools dropdown menus, and the right-aligned asbplayer / mpv
	// / Anki status indicators. Entries whose modal/command isn't built yet are
	// rendered disabled with a "coming soon" tooltip; each is enabled by its own
	// task (see the per-item comments), at which point it gains an `onclick`.
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import {
		settings,
		ankiStatus,
		playerStatus,
		languageToolsStatus,
		fileResult,
		toggleDarkMode,
		toggleSerifFont,
		openAndProcessFile,
		openAnkiModal,
		openIgnoreModal,
		openWebsocketModal,
		openFrequencyModal,
		openPosModal,
		openSetupModal,
		openAnalyzerModal,
		openDataFolder,
		loadFrequencyDictionaries,
		refreshTerms
	} from '$lib/stores';

	type MenuName = 'file' | 'settings' | 'tools';
	let openMenu = $state<MenuName | null>(null);

	const toolsReady = $derived($languageToolsStatus === 'ready');
	const isDark = $derived($settings?.dark_mode ?? true);
	const isSerif = $derived($settings?.use_serif_font ?? false);

	function toggleMenu(menu: MenuName, event: MouseEvent) {
		// Stop the window handler (which closes menus) from firing on this same click.
		event.stopPropagation();
		openMenu = openMenu === menu ? null : menu;
	}

	function run(action: () => void) {
		action();
		openMenu = null;
	}

	function quit() {
		getCurrentWindow().close();
	}

	// --- Status indicators (egui `show_status_indicators`). PlayerStatus now carries
	// the WebSocket `server_state` (+ error message), so the asbplayer dot mirrors
	// egui's full mapping: green has-clients, yellow Running, blue Starting,
	// red Error (tooltip = message), grey Stopped (T056). -----------------------
	const GREEN = '#00c800';
	const YELLOW = '#c8c800';
	const BLUE = '#6464c8';
	const RED = '#c80000';
	const GREY = '#646464';
	const ANKI_RED = '#c85050';

	const asbplayer = $derived.by(() => {
		const s = $playerStatus;
		if (s.server_state === 'running' && s.ws_clients > 0)
			return { color: GREEN, tip: 'Connected to asbplayer' };
		if (s.server_state === 'running')
			return { color: YELLOW, tip: 'WebSocket server running - waiting for asbplayer' };
		if (s.server_state === 'error')
			return { color: RED, tip: `WebSocket server error: ${s.server_error ?? ''}` };
		if (s.server_state === 'starting') return { color: BLUE, tip: 'WebSocket server starting...' };
		return { color: GREY, tip: 'WebSocket server stopped' };
	});

	const mpv = $derived(
		$playerStatus.mpv_connected
			? { color: GREEN, tip: 'MPV detected - using MPV mode' }
			: { color: GREY, tip: 'MPV not detected' }
	);

	const ankiTip = $derived(
		$ankiStatus.fetching
			? 'Syncing with Anki...'
			: $ankiStatus.connected
				? 'Connected to Anki'
				: 'Not Connected to Anki'
	);
</script>

<!-- Any click outside an open menu closes it. -->
<svelte:window onclick={() => (openMenu = null)} />

<header class="topbar">
	<span class="brand">Yomine</span>

	<button
		class="icon-btn"
		title={isDark ? 'Switch to light mode' : 'Switch to dark mode'}
		onclick={toggleDarkMode}>{isDark ? '☀' : '🌙'}</button
	>
	<button
		class="icon-btn"
		title={isSerif ? 'Switch to Sans' : 'Switch to Serif'}
		onclick={toggleSerifFont}>字</button
	>

	<span class="sep"></span>

	<div class="menu" class:open={openMenu === 'file'}>
		<button class="menu-trigger" onclick={(e) => toggleMenu('file', e)}>File</button>
		{#if openMenu === 'file'}
			<div class="menu-panel">
				<button onclick={() => run(openAndProcessFile)} disabled={!toolsReady}
					>Open New File</button
				>
				<button onclick={() => run(loadFrequencyDictionaries)} disabled={!toolsReady}
					>Load New Frequency Dictionaries</button
				>
				<button onclick={() => run(openDataFolder)}>Open Data Folder</button>
				<button onclick={() => run(quit)}>Quit</button>
			</div>
		{/if}
	</div>

	<div class="menu" class:open={openMenu === 'settings'}>
		<button class="menu-trigger" onclick={(e) => toggleMenu('settings', e)}>Settings</button>
		{#if openMenu === 'settings'}
			<div class="menu-panel">
				<button onclick={() => run(openAnkiModal)}>Anki</button>
				<button onclick={() => run(openWebsocketModal)}>WebSocket Server</button>
				<button onclick={() => run(openIgnoreModal)} disabled={!toolsReady}>Ignore List</button>
				<button onclick={() => run(openFrequencyModal)}>Frequency Weighting</button>
				<button onclick={() => run(openPosModal)}>Part of Speech Filters</button>
				<button onclick={() => run(openSetupModal)}>Setup Checklist</button>
			</div>
		{/if}
	</div>

	<div class="menu" class:open={openMenu === 'tools'}>
		<button class="menu-trigger" onclick={(e) => toggleMenu('tools', e)}>Tools</button>
		{#if openMenu === 'tools'}
			<div class="menu-panel">
				<button onclick={() => run(openAnalyzerModal)} disabled={!toolsReady}
					>Frequency Analyzer</button
				>
			</div>
		{/if}
	</div>

	<!-- Reapply ignorelist + Anki filters (egui's 🔄, T033); only meaningful with
	     a file loaded. Keyboard: F5 / Ctrl+R (wired app-wide in +page). -->
	{#if $fileResult}
		<button
			class="icon-btn"
			title="Reapply ignorelist and Anki filters (F5 / Ctrl+R)"
			disabled={!toolsReady}
			onclick={refreshTerms}>🔄</button
		>
	{/if}

	<span class="spacer"></span>

	<div class="status">
		<span class="indicator" title={asbplayer.tip}>
			<small>asbplayer</small>
			<span class="dot" style:color={asbplayer.color}>●</span>
		</span>
		<span class="indicator" title={mpv.tip}>
			<small>mpv</small>
			<span class="dot" style:color={mpv.color}>●</span>
		</span>
		<span class="indicator" title={ankiTip}>
			<small>Anki</small>
			{#if $ankiStatus.fetching}
				<span class="spinner" aria-label="Syncing with Anki"></span>
			{:else}
				<span class="dot" style:color={$ankiStatus.connected ? GREEN : ANKI_RED}>●</span>
			{/if}
		</span>
	</div>
</header>

<style>
	.topbar {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.35rem 1rem;
		background: var(--bg-dark);
		border-bottom: 1px solid var(--border);
	}
	.brand {
		font-weight: 700;
		font-size: 1.05rem;
		margin-right: 0.4rem;
	}
	.icon-btn {
		padding: 0.2rem 0.45rem;
		background: transparent;
		border: none;
		font-size: 0.95rem;
		line-height: 1;
		border-radius: var(--radius);
	}
	.icon-btn:hover {
		background: var(--bg-light);
	}
	.sep {
		width: 1px;
		align-self: stretch;
		margin: 0.2rem 0.25rem;
		background: var(--border);
	}
	.menu {
		position: relative;
	}
	.menu-trigger {
		padding: 0.25rem 0.55rem;
		background: transparent;
		border: none;
		border-radius: var(--radius);
	}
	.menu-trigger:hover,
	.menu.open .menu-trigger {
		background: var(--bg-light);
	}
	.menu-panel {
		position: absolute;
		top: 100%;
		left: 0;
		z-index: 30;
		margin-top: 0.2rem;
		min-width: 220px;
		display: flex;
		flex-direction: column;
		padding: 0.25rem;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
	}
	.menu-panel button {
		text-align: left;
		padding: 0.4rem 0.6rem;
		background: transparent;
		border: none;
		border-radius: var(--radius);
		white-space: nowrap;
	}
	.menu-panel button:hover:not(:disabled) {
		background: var(--bg-lighter);
	}
	.menu-panel button:disabled {
		color: var(--comment);
		cursor: default;
	}
	.spacer {
		flex: 1;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}
	.indicator {
		display: inline-flex;
		align-items: center;
		gap: 0.2rem;
	}
	.indicator small {
		font-size: 0.7rem;
		color: var(--comment);
	}
	.dot {
		font-size: 0.7rem;
		line-height: 1;
	}
	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid var(--comment);
		border-top-color: var(--cyan);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
