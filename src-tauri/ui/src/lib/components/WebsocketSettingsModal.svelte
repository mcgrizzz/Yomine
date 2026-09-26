<script lang="ts">
	// Staged edits; Cancel reverts but keeps the modal open (egui behavior).
	// Saving the port also restarts a running server on it.
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import {
		defaultSettings,
		settings,
		websocketModalOpen,
		saveWebsocketPort,
		setAsbplayerPollSecs
	} from '$lib/stores';
	import type { SettingsData } from '$lib/ipc';

	const fields = (s: SettingsData) => ({
		port: s.websocket_settings.port,
		poll: s.asbplayer_poll_secs
	});
	const form = settingsDraft({
		open: websocketModalOpen,
		initial: { port: 0, poll: 0 },
		load: () => {
			const s = $settings ?? $defaultSettings;
			return s ? fields(s) : undefined;
		}
	});
	const draft = $derived(form.value);

	// u16 caps at 65535 — the number input doesn't.
	const valid = $derived(Number.isInteger(draft.port) && draft.port >= 1024 && draft.port <= 65535);
	const pollValid = $derived(Number.isInteger(draft.poll) && draft.poll >= 1 && draft.poll <= 60);

	async function save() {
		if (draft.poll !== form.saved.poll) {
			await setAsbplayerPollSecs(draft.poll);
			form.saved.poll = draft.poll;
		}
		if (draft.port !== form.saved.port) {
			if (!(await saveWebsocketPort(draft.port))) return;
			// On failure the lastError banner shows; staged state stays for a retry.
		}
		websocketModalOpen.set(false);
	}

	function restoreDefault() {
		if ($defaultSettings) form.value = fields($defaultSettings);
	}
</script>

<Modal
	open={$websocketModalOpen}
	title="WebSocket Server Settings"
	width="min(420px, 92%)"
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="port-row">
		<label for="ws-port">Server Port:</label>
		<input id="ws-port" type="number" min="1024" max="65535" bind:value={draft.port} />
		<span class="hint">(Valid range: 1024-65535)</span>
	</div>
	{#if !valid}
		<p class="invalid">⚠ Port must be between 1024 and 65535</p>
	{/if}

	<div class="port-row">
		<label for="asb-poll">asbplayer poll interval:</label>
		<input id="asb-poll" type="number" min="1" max="60" bind:value={draft.poll} />
		<span class="hint">seconds (1-60; used by follow mode)</span>
	</div>
	{#if !pollValid}
		<p class="invalid">⚠ Poll interval must be between 1 and 60 seconds</p>
	{/if}

	{#snippet footer()}
		<SettingsFooter
			{form}
			invalid={!valid || !pollValid}
			onsave={save}
			oncancel={form.revert}
			onrestore={restoreDefault}
		/>
	{/snippet}
</Modal>

<style>
	.port-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.port-row input {
		width: 6.5rem;
		padding: 0.3rem 0.5rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.hint {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.invalid {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--danger);
	}
</style>
