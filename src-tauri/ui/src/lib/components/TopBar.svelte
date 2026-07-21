<script lang="ts">
	// Menu grouping deviates from egui: Mining = what you tweak while working;
	// Settings = configuration + the setup checklist.
	import { openUrl } from '@tauri-apps/plugin-opener';
	import {
		settings,
		ankiStatus,
		asbContext,
		playerStatus,
		languageToolsStatus,
		updateInfo,
		installUpdate,
		fileResult,
		toggleDarkMode,
		toggleSerifFont,
		openAndProcessFile,
		openAnkiModal,
		openIgnoreModal,
		openTextFiltersModal,
		openWebsocketModal,
		openAppearanceModal,
		openAsbplayerModal,
		openFrequencyModal,
		openPosModal,
		openSetupModal,
		openAnalyzerModal,
		openAboutModal,
		openDataFolder,
		refreshTerms,
		setAsbplayerFollowNewMedia,
		setAsbplayerFollowActiveTab,
		launchMpvVideo,
		locateMpvAndRetry,
		mpvLocatePrompt,
		yomitanReachable
	} from '$lib/stores';
	import { openThemesWindow } from '$lib/ipc';

	type MenuName = 'file' | 'mining' | 'appearance' | 'settings' | 'asb' | 'mpv';
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

	const GREEN = 'var(--success)';
	const YELLOW = 'var(--warning)';
	const BLUE = 'var(--info)';
	const RED = 'var(--danger)';
	const GREY = 'var(--text-muted)';
	const ANKI_RED = 'var(--danger)';

	const asbplayer = $derived.by(() => {
		const s = $playerStatus;
		if (s.server_state === 'running' && s.ws_clients > 0) {
			if ($asbContext.loaded_from_asbplayer && !$asbContext.loaded_is_active)
				return {
					color: YELLOW,
					tip: "asbplayer's active tab is not the loaded video — mined media would come from the wrong one"
				};
			return {
				color: GREEN,
				tip: 'asbplayer mode — seeking and card media capture while mining'
			};
		}
		if (s.server_state === 'running')
			return { color: YELLOW, tip: 'WebSocket server running - waiting for asbplayer' };
		if (s.server_state === 'error')
			return { color: RED, tip: `WebSocket server error: ${s.server_error ?? ''}` };
		if (s.server_state === 'starting') return { color: BLUE, tip: 'WebSocket server starting...' };
		return { color: GREY, tip: 'WebSocket server stopped' };
	});

	// Bound-media polling only runs for follow mode or an asbplayer session, so
	// the panel's active-tab note is stale outside those.
	const asbPolled = $derived(
		($settings?.asbplayer_follow_new_media ?? false) ||
			($settings?.asbplayer_follow_active_tab ?? false) ||
			$asbContext.loaded_from_asbplayer
	);

	const mpv = $derived(
		$playerStatus.mpv_connected
			? {
					color: GREEN,
					tip: 'MPV mode — seeking works, but mined cards get no audio/screenshot (media capture needs asbplayer)'
				}
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

<!-- Any click outside an open menu closes it; Esc too. -->
<svelte:window
	onclick={() => (openMenu = null)}
	onkeydown={(e) => e.key === 'Escape' && (openMenu = null)}
/>

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
				<button
					onclick={() => run(openAsbplayerModal)}
					disabled={!toolsReady || $playerStatus.ws_clients === 0}
					title={$playerStatus.ws_clients === 0 ? 'asbplayer is not connected' : undefined}
					>Load from asbplayer…</button
				>
				<div class="menu-sep"></div>
				<button onclick={() => run(openDataFolder)}>Open Data Folder</button>
				<button onclick={() => run(openAboutModal)}>About Yomine</button>
			</div>
		{/if}
	</div>

	<!-- Mining data + tools: what you tweak while working through a file. -->
	<div class="menu" class:open={openMenu === 'mining'}>
		<button class="menu-trigger" onclick={(e) => toggleMenu('mining', e)}>Mining</button>
		{#if openMenu === 'mining'}
			<div class="menu-panel">
				<button onclick={() => run(openIgnoreModal)} disabled={!toolsReady}>Ignore List</button>
				<button onclick={() => run(openPosModal)}>Part of Speech Filters</button>
				<button onclick={() => run(openTextFiltersModal)}>Text Filters</button>
				<button onclick={() => run(openFrequencyModal)}>Frequency Dictionaries</button>
				<div class="menu-sep"></div>
				<button onclick={() => run(openAnalyzerModal)} disabled={!toolsReady}
					>Frequency Analyzer</button
				>
			</div>
		{/if}
	</div>

	<div class="menu" class:open={openMenu === 'appearance'}>
		<button class="menu-trigger" onclick={(e) => toggleMenu('appearance', e)}>Appearance</button>
		{#if openMenu === 'appearance'}
			<div class="menu-panel">
				<button onclick={() => run(() => void openThemesWindow())}>Themes</button>
				<button onclick={() => run(openAppearanceModal)}>General</button>
			</div>
		{/if}
	</div>

	<!-- True configuration: integrations, plus the onboarding checklist. -->
	<div class="menu" class:open={openMenu === 'settings'}>
		<button class="menu-trigger" onclick={(e) => toggleMenu('settings', e)}>Settings</button>
		{#if openMenu === 'settings'}
			<div class="menu-panel">
				<button onclick={() => run(openAnkiModal)}>Anki</button>
				<button onclick={() => run(openWebsocketModal)}>WebSocket Server</button>
				<div class="menu-sep"></div>
				<button onclick={() => run(openSetupModal)}>Setup Checklist</button>
			</div>
		{/if}
	</div>

	<!-- Reapply ignorelist + Anki filters (egui's 🔄); only meaningful with
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

	{#if $updateInfo}
		{@const u = $updateInfo}
		<button
			class="update-pill"
			title={u.installable
				? `Yomine ${u.latest} is available (you have v${u.current}) — click to download and install`
				: `Yomine ${u.latest} is available (you have v${u.current}) — open the release page`}
			onclick={() => (u.installable ? installUpdate() : openUrl(u.url))}
		>
			⬆ {u.latest} available
		</button>
	{/if}

	<div class="status">
		<!-- The asbplayer indicator doubles as the follow-mode menu (issue #105). -->
		<div class="menu" class:open={openMenu === 'asb'}>
			<button
				class="indicator status-trigger"
				class:active-mode={$playerStatus.mode === 'asbplayer' && $playerStatus.ws_clients > 0}
				title={asbplayer.tip}
				onclick={(e) => toggleMenu('asb', e)}
			>
				<small>asbplayer</small>
				<span class="dot" style:color={asbplayer.color}>●</span>
			</button>
			{#if openMenu === 'asb'}
				<!-- Toggling a follow checkbox keeps the menu open; Esc / clicking
				     elsewhere closes it (the stopPropagation shields the window handler). -->
				<div
					class="menu-panel right"
					role="menu"
					tabindex="-1"
					onclick={(e) => e.stopPropagation()}
					onkeydown={(e) => e.key === 'Escape' && (openMenu = null)}
				>
					{#if $playerStatus.ws_clients > 0 && asbPolled}
						<span class="menu-note">
							{$asbContext.active_title
								? `Active tab: ${$asbContext.active_title}${$asbContext.active_has_subtitles ? '' : ' — no subtitles'}`
								: 'No active tab in asbplayer'}
						</span>
						{#if $asbContext.loaded_from_asbplayer && !$asbContext.loaded_is_active}
							<span class="menu-note warn">Loaded video is not the active tab</span>
						{/if}
					{/if}
					<button
						onclick={() => run(openAsbplayerModal)}
						disabled={!toolsReady || $playerStatus.ws_clients === 0}
						>Load from asbplayer…</button
					>
					<label
						class="menu-check"
						title="Automatically load new videos asbplayer picks up (e.g. the next episode)."
					>
						<input
							type="checkbox"
							checked={$settings?.asbplayer_follow_new_media ?? false}
							onchange={(e) => setAsbplayerFollowNewMedia(e.currentTarget.checked)}
						/>
						Follow new videos
					</label>
					<label
						class="menu-check"
						title="Switch to the active tab's video (with subtitles) when it isn't the loaded one."
					>
						<input
							type="checkbox"
							checked={$settings?.asbplayer_follow_active_tab ?? false}
							onchange={(e) => setAsbplayerFollowActiveTab(e.currentTarget.checked)}
						/>
						Follow active tab
					</label>
				</div>
			{/if}
		</div>
		<!-- The mpv indicator doubles as the launcher menu (issue #89). -->
		<div class="menu" class:open={openMenu === 'mpv'}>
			<button
				class="indicator status-trigger"
				class:active-mode={$playerStatus.mpv_connected}
				title={mpv.tip}
				onclick={(e) => toggleMenu('mpv', e)}
			>
				<small>mpv</small>
				<span class="dot" style:color={mpv.color}>●</span>
			</button>
			{#if openMenu === 'mpv'}
				<div
					class="menu-panel right"
					role="menu"
					tabindex="-1"
					onclick={(e) => e.stopPropagation()}
					onkeydown={(e) => e.key === 'Escape' && (openMenu = null)}
				>
					<span class="menu-note">{mpv.tip}</span>
					<!-- Not run(): the panel must stay open so the not-found row can appear. -->
					<button
						onclick={async () => {
							if (await launchMpvVideo()) openMenu = null;
						}}
						disabled={$playerStatus.mpv_connected}
						title={$playerStatus.mpv_connected
							? 'MPV is already connected — seeking uses the running instance'
							: 'Pick a video file and open it in MPV, ready for seeking'}
						>Launch video in MPV…</button
					>
					{#if $mpvLocatePrompt}
						<span class="menu-note warn">mpv not found (tried “{$settings?.mpv_path}”)</span>
						<button
							onclick={async () => {
								if (await locateMpvAndRetry()) openMenu = null;
							}}>Locate mpv…</button
						>
					{/if}
				</div>
			{/if}
		</div>
		<span class="indicator" title={ankiTip}>
			<small>Anki</small>
			{#if $ankiStatus.fetching}
				<span class="spinner" aria-label="Syncing with Anki"></span>
			{:else}
				<span class="dot" style:color={$ankiStatus.connected ? GREEN : ANKI_RED}>●</span>
			{/if}
		</span>
		<!-- Optional (grey, not red, when absent) — gates the one-click mine buttons. -->
		<span
			class="indicator"
			title={$yomitanReachable
				? 'Yomitan API connected — one-click mining available'
				: 'Yomitan API not detected — one-click mining disabled'}
		>
			<small>Yomitan</small>
			<span class="dot" style:color={$yomitanReachable ? GREEN : GREY}>●</span>
		</span>
	</div>
</header>

<style>
	.topbar {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.35rem 1rem;
		background: var(--bg-panel);
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
		background: var(--bg-raised);
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
		background: var(--bg-raised);
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
		background: var(--bg-raised);
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
		background: var(--bg-hover);
	}
	.menu-panel button:disabled {
		color: var(--text-muted);
		cursor: default;
	}
	.menu-sep {
		height: 1px;
		margin: 0.25rem 0.4rem;
		background: var(--border);
	}
	.spacer {
		flex: 1;
	}
	.update-pill {
		margin-right: 0.4rem;
		padding: 0.15rem 0.55rem;
		font-size: 0.78rem;
		color: var(--success);
		background: transparent;
		border: 1px solid var(--success);
		border-radius: 999px;
		white-space: nowrap;
	}
	.update-pill:hover {
		background: color-mix(in srgb, var(--success) 15%, transparent);
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
	/* Status indicators that are real buttons (they open a menu panel). */
	.status-trigger {
		padding: 0.15rem 0.35rem;
		background: transparent;
		border: none;
		border-radius: var(--radius);
		color: inherit;
		font: inherit;
		cursor: pointer;
	}
	.status-trigger:hover,
	.menu.open .status-trigger {
		background: var(--bg-raised);
	}
	.menu-note {
		padding: 0.4rem 0.6rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		white-space: nowrap;
	}
	.menu-note.warn {
		color: var(--warning);
	}
	/* The player currently driving seek/mining — a soft pill marks the mode. */
	.status-trigger.active-mode {
		background: color-mix(in srgb, var(--accent) 13%, transparent);
	}
	.status-trigger.active-mode small {
		color: var(--accent);
	}
	/* Right-anchored panel so it doesn't overflow the window edge. */
	.menu-panel.right {
		left: auto;
		right: 0;
	}
	.menu-check {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.4rem 0.6rem;
		font-size: 0.85rem;
		white-space: nowrap;
		cursor: pointer;
		border-radius: var(--radius);
	}
	.menu-check:hover {
		background: var(--bg-hover);
	}
	.indicator small {
		font-size: 0.7rem;
		color: var(--text-muted);
	}
	.dot {
		font-size: 0.7rem;
		line-height: 1;
	}
	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid var(--text-muted);
		border-top-color: var(--accent);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
