<script lang="ts">
	import { untrack } from 'svelte';
	import Modal from './Modal.svelte';
	import {
		openExternal,
		setupModalOpen,
		setupStatus,
		refreshSetupStatus,
		openAnkiModal,
		openWebsocketModal,
		openFrequencyModal,
		ankiModalOpen,
		websocketModalOpen,
		frequencyModalOpen,
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

	const actionModalOpen = $derived($ankiModalOpen || $websocketModalOpen || $frequencyModalOpen);

	// untrack: the refresh reads stores that must not re-trigger this effect.
	$effect(() => {
		if ($setupModalOpen && !actionModalOpen) untrack(() => refreshSetupStatus());
	});

	function s(complete: boolean): ItemStatus {
		return complete ? 'complete' : 'incomplete';
	}

	const items = $derived.by<CheckItem[]>(() => {
		const st = $setupStatus;
		const mappingsEmpty = !$settings || Object.keys($settings.anki_model_mappings).length === 0;
		const count = st?.frequency_dict_count ?? 0;

		// Only AnkiConnect has a helpUrl: the README anchors the others pointed at no
		// longer exist. Restore them against the docs site, not the README.
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
				action: openFrequencyModal,
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
				helpUrl: null,
				action: openAnkiModal,
				actionText: 'Setup Anki'
			},
			{
				title: 'asbplayer or mpv detected',
				description: 'Required for video timestamp integration',
				status: s(st?.player_connected ?? false),
				optional: false,
				helpUrl: null,
				action: openWebsocketModal,
				actionText: 'Configure WebSocket'
			},
			{
				title: 'Yomitan API Detected [Optional]',
				description: 'Enables one-click mining — Anki cards rendered with your Yomitan templates',
				status: s(st?.yomitan_connected ?? false),
				optional: true,
				helpUrl: null,
				action: openAnkiModal,
				actionText: 'Configure URL'
			},
			{
				title: 'Additional Frequency Dictionaries Installed [Optional]',
				description: 'Load additional dictionaries via Mining → Frequency Dictionaries',
				status: s(count > 1),
				optional: true,
				helpUrl: null,
				action: openFrequencyModal,
				actionText: '+ Install Dictionary'
			}
		];
	});

	function iconFor(item: CheckItem): { icon: string; cls: string } {
		if (item.status === 'complete') return { icon: '✓', cls: 'complete' };
		if (item.optional) return { icon: '◯', cls: 'optional' };
		return { icon: '✕', cls: 'required' };
	}

	function close() {
		setupModalOpen.set(false);
	}
</script>

<Modal
	open={$setupModalOpen}
	title="Setup Checklist"
	width="min(600px, 92%)"
	flush
	onclose={close}
>
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
						<button onclick={() => item.action?.()}>{item.actionText}</button>
					{/if}
					{#if item.helpUrl}
						<button onclick={() => openExternal(item.helpUrl!)}>📖 View Docs</button>
					{/if}
				</div>
			</li>
		{/each}
	</ul>

	{#snippet footer()}
		<footer>
			<button onclick={close}>Close</button>
		</footer>
	{/snippet}
</Modal>

<style>
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
		color: var(--text-muted);
	}
	.complete {
		color: var(--success);
	}
	.required {
		color: var(--danger);
	}
	.optional {
		color: var(--text-muted);
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
	footer {
		display: flex;
		justify-content: flex-end;
		padding: 0.75rem 1rem;
		border-top: 1px solid var(--border);
	}
</style>
