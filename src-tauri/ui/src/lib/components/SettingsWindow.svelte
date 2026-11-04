<script lang="ts">
	import type { Snippet } from 'svelte';
	import Modal from './Modal.svelte';
	import {
		closeSettings,
		openSettings,
		settingsHidden,
		settingsPage,
		type SettingsPage
	} from '$lib/stores';

	/** The settings dialogs, each shown as a page while it's open. */
	let { children }: { children: Snippet } = $props();

	const CONNECTIONS: [SettingsPage, string][] = [
		['anki', 'Anki'],
		['local-media', 'Local media'],
		['websocket', 'WebSocket server']
	];
</script>

{#snippet item(page: SettingsPage, label: string)}
	<button
		class="item"
		class:on={$settingsPage === page}
		aria-current={$settingsPage === page ? 'page' : undefined}
		onclick={() => openSettings(page)}>{label}</button
	>
{/snippet}

<Modal
	open={$settingsPage !== null}
	hidden={$settingsHidden}
	title="Settings"
	width="min(900px, 94%)"
	height="85%"
	flush
	onclose={closeSettings}
>
	<div class="window">
		<nav aria-label="Settings">
			{@render item('appearance', 'Appearance')}
			<span class="gap"></span>
			<span class="group" role="presentation">Connections</span>
			{#each CONNECTIONS as [page, label] (page)}
				{@render item(page, label)}
			{/each}
			<span class="gap"></span>
			{@render item('mining', 'Mining')}
			{@render item('matching', 'Show matching')}
		</nav>
		<div class="pages">{@render children()}</div>
	</div>
</Modal>

<style>
	.window {
		display: flex;
		flex: 1;
		min-height: 0;
	}
	nav {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: 1px;
		width: 11rem;
		padding: 0.6rem 0.4rem;
		border-right: 1px solid var(--border);
		overflow-y: auto;
	}
	.group {
		padding: 0.2rem 0.6rem 0.3rem;
		font-size: 0.72rem;
		font-weight: 600;
		color: var(--text-muted);
	}
	.gap {
		height: 0.6rem;
	}
	.item {
		padding: 0.3rem 0.6rem;
		text-align: left;
		color: var(--text);
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
	}
	.item:hover:not(.on) {
		background: var(--bg-hover);
	}
	.item.on {
		background: color-mix(in srgb, var(--accent) 16%, var(--bg-panel));
		box-shadow: inset 2px 0 0 var(--accent);
	}
	.pages {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		padding-top: 0.25rem;
	}
</style>
