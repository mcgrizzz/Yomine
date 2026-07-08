<script lang="ts">
	import { untrack } from 'svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import {
		setupModalOpen,
		setupStatus,
		refreshSetupStatus,
		openAnkiModal,
		openWebsocketModal,
		loadFrequencyDictionaries,
		settings
	} from '$lib/stores';

	type ItemStatus = 'complete' | 'incomplete';

	interface CheckItem {
		title: string;
		description: string;
		status: ItemStatus;
		optional: boolean;
		helpUrl: string | null;
		action: (() => void) | null;
		actionText: string | null;
	}

	// untrack: the refresh reads stores that must not re-trigger this effect.
	$effect(() => {
		if ($setupModalOpen) untrack(() => refreshSetupStatus());
	});

	function s(complete: boolean): ItemStatus {
		return complete ? 'complete' : 'incomplete';
	}

	// The two "Install Dictionary" actions (items 2 & 6) run the
	// freq-dictionary zip import.
	const items = $derived.by<CheckItem[]>(() => {
		const st = $setupStatus;
		const mappingsEmpty = !$settings || Object.keys($settings.anki_model_mappings).length === 0;
		const count = st?.frequency_dict_count ?? 0;

		return [
			{
				title: 'Tokenizer Installed',
				description: 'Required for Japanese text segmentation',
				status: s(st?.tools_loaded ?? false),
				optional: false,
				helpUrl: null,
				action: null,
				actionText: null
			},
			{
				title: 'Default Frequency Dictionary Installed',
				description: 'Auto-downloads on first run',
				status: s((st?.has_frequency_dict ?? false) && count >= 1),
				optional: false,
				helpUrl: null,
				action: loadFrequencyDictionaries,
				actionText: '+ Install Dictionary'
			},
			{
				title: 'AnkiConnect Enabled and Detected',
				description: 'Required for Anki integration',
				status: s(st?.anki_connected ?? false),
				optional: false,
				helpUrl: 'https://ankiweb.net/shared/info/2055492159',
				action: null,
				actionText: null
			},
			{
				title: 'Anki Notetypes Setup',
				description: 'Required for Anki integration',
				status: s(!mappingsEmpty),
				optional: false,
				helpUrl:
					'https://github.com/mcgrizzz/Yomine?tab=readme-ov-file#setting-up-anki-integration',
				action: openAnkiModal,
				actionText: 'Setup Anki'
			},
			{
				title: 'asbplayer or mpv detected',
				description: 'Required for video timestamp integration',
				status: s(st?.player_connected ?? false),
				optional: false,
				helpUrl:
					'https://github.com/mcgrizzz/Yomine?tab=readme-ov-file#configuring-websocket-connection',
				action: openWebsocketModal,
				actionText: 'Configure WebSocket'
			},
			{
				title: 'Additional Frequency Dictionaries Installed [Optional]',
				description: 'Load additional dictionaries via Mining → Frequency Dictionaries',
				status: s(count > 1),
				optional: true,
				helpUrl:
					'https://github.com/mcgrizzz/Yomine?tab=readme-ov-file#setting-up-frequency-dictionaries',
				action: loadFrequencyDictionaries,
				actionText: '+ Install Dictionary'
			}
		];
	});

	function iconFor(item: CheckItem): { icon: string; cls: string } {
		if (item.status === 'complete') return { icon: '✓', cls: 'complete' };
		if (item.optional) return { icon: '◯', cls: 'optional' };
		return { icon: '✕', cls: 'required' };
	}

	function runAction(item: CheckItem) {
		item.action?.();
		setupModalOpen.set(false); // egui closes the checklist after an action.
	}

	function viewDocs(url: string) {
		openUrl(url);
	}

	function close() {
		setupModalOpen.set(false);
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window onkeydown={(e) => $setupModalOpen && e.key === 'Escape' && close()} />

{#if $setupModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={close}
		onkeydown={(e) => e.key === 'Escape' && close()}
	>
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Setup checklist"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Setup Checklist</h2>
				<button class="close" aria-label="Close" onclick={close}>✕</button>
			</header>

			<ul class="items">
				{#each items as item (item.title)}
					{@const ic = iconFor(item)}
					<li class="item">
						<span class="icon {ic.cls}">{ic.icon}</span>
						<div class="text">
							<span class="title {ic.cls}">{item.title}</span>
							<span class="desc">{item.description}</span>
						</div>
						<div class="actions">
							{#if item.action || item.actionText}
								<button onclick={() => runAction(item)}>{item.actionText}</button>
							{/if}
							{#if item.helpUrl}
								<button class="docs" onclick={() => viewDocs(item.helpUrl!)}>📖 View Docs</button>
							{/if}
						</div>
					</li>
				{/each}
			</ul>

			<footer>
				<button onclick={close}>Close</button>
			</footer>
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
		background: color-mix(in srgb, var(--bg-darker) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		width: min(600px, 92vw);
		max-height: 85vh;
		background: var(--bg-dark);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--border);
	}
	header h2 {
		margin: 0;
		font-size: 1.05rem;
		color: var(--cyan);
	}
	.close {
		padding: 0.1rem 0.4rem;
	}
	.items {
		list-style: none;
		margin: 0;
		padding: 0.5rem 1rem;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.item {
		display: flex;
		align-items: flex-start;
		gap: 0.6rem;
	}
	.icon {
		font-size: 1.2rem;
		line-height: 1.3;
		width: 1.4rem;
		text-align: center;
		flex-shrink: 0;
	}
	.text {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
		flex: 1;
		min-width: 0;
	}
	.title {
		font-weight: 600;
	}
	.desc {
		font-size: 0.8rem;
		color: var(--comment);
	}
	/* Status colors mirror egui: green complete, red incomplete-required,
	   grey incomplete-optional. */
	.complete {
		color: #00c800;
	}
	.required {
		color: #c85050;
	}
	.optional {
		color: #969696;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		flex-shrink: 0;
	}
	.actions button {
		padding: 0.25rem 0.5rem;
		font-size: 0.85rem;
		white-space: nowrap;
	}
	.actions button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	footer {
		display: flex;
		justify-content: flex-end;
		padding: 0.75rem 1rem;
		border-top: 1px solid var(--border);
	}
</style>
