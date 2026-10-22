<script lang="ts">
	// Staged edits; Cancel reverts but keeps the modal open (egui behavior).
	// Saving the port also restarts a running server on it.
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import SettingsSection from './SettingsSection.svelte';
	import {
		defaultSettings,
		playerStatus,
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
	/** The port the server runs on, which changes only on Save. */
	const activePort = $derived($settings?.websocket_settings.port ?? form.saved.port);

	const SERVER = {
		running: { label: 'Listening', color: 'var(--status-ok)' },
		starting: { label: 'Starting', color: 'var(--status-busy)' },
		error: { label: 'Error', color: 'var(--status-error)' },
		stopped: { label: 'Stopped', color: 'var(--status-off)' }
	};
	const server = $derived(SERVER[$playerStatus.server_state]);
	const serverDetail = $derived.by(() => {
		const { server_state: state, ws_clients: clients, server_error: error } = $playerStatus;
		if (state === 'error') return error ? `: ${error}` : '';
		if (state === 'stopped') return '';
		return ` on port ${activePort}${state === 'running' && clients > 0 ? ' · asbplayer connected' : ''}`;
	});

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
	title="WebSocket server"
	width="min(420px, 92%)"
	page
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<SettingsSection title="Server connection">
			<p class="server" role="status">
				<span class="dot" style:color={server.color} aria-hidden="true">●</span>
				{server.label}{serverDetail}
			</p>
			<div class="fields">
				<label for="ws-port">Port</label>
				<input
					id="ws-port"
					class="port"
					type="number"
					min="1024"
					max="65535"
					aria-invalid={!valid}
					aria-describedby="ws-port-help"
					bind:value={draft.port}
				/>
				<div id="ws-port-help" class="help">
					{#if !valid}
						<p class="error">Enter a whole number from 1024 to 65535.</p>
					{:else if draft.port !== activePort}
						<p>
							Not saved yet. Saving moves a running server to port {draft.port} right away; set asbplayer's
							WebSocket address to <code>ws://127.0.0.1:{draft.port}</code> too.
						</p>
					{:else}
						<p>
							asbplayer connects to <code>ws://127.0.0.1:{activePort}</code>. From 1024 to 65535.
						</p>
					{/if}
				</div>
			</div>
		</SettingsSection>

		<SettingsSection title="asbplayer follow mode">
			<div class="fields">
				<label for="asb-poll">Polling interval</label>
				<span class="with-unit">
					<input
						id="asb-poll"
						class="poll"
						type="number"
						min="1"
						max="60"
						aria-invalid={!pollValid}
						aria-describedby="asb-poll-help"
						bind:value={draft.poll}
					/>
					seconds
				</span>
				<div id="asb-poll-help" class="help">
					{#if !pollValid}
						<p class="error">Enter a whole number from 1 to 60.</p>
					{/if}
					<p>
						In asbplayer mode, how often Yomine asks asbplayer which videos are open. Following new
						videos or the active tab happens on this check, so a shorter interval reacts sooner.
					</p>
				</div>
			</div>
		</SettingsSection>
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			invalid={!valid || !pollValid}
			onsave={save}
			oncancel={form.revert}
			restore={{
				label: 'Reset WebSocket Settings',
				title: 'Restore the server port and the asbplayer poll interval to their defaults.',
				run: restoreDefault
			}}
		/>
	{/snippet}
</Modal>

<style>
	.body {
		display: grid;
		gap: 1.25rem;
		padding: 0 1rem;
	}
	p {
		margin: 0;
	}
	.server {
		font-size: 0.85rem;
		color: var(--text-muted);
		overflow-wrap: anywhere;
	}
	.dot {
		font-size: 0.6rem;
		margin-right: 0.3rem;
	}
	/* Shared by both sections, so their inputs line up. */
	.fields {
		display: grid;
		grid-template-columns: 8.5rem minmax(0, 1fr);
		align-items: center;
		gap: 0.35rem 0.9rem;
	}
	label {
		font-size: 0.85rem;
	}
	.port {
		justify-self: start;
		width: 6.5rem;
	}
	.poll {
		width: 4.5rem;
	}
	.with-unit {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.85rem;
	}
	.help {
		display: grid;
		gap: 0.2rem;
		grid-column: 2;
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--text-muted);
	}
	.error {
		color: var(--danger);
	}
	code {
		overflow-wrap: anywhere;
	}
	@media (max-width: 640px) {
		.fields {
			grid-template-columns: minmax(0, 1fr);
		}
		.help {
			grid-column: 1;
		}
	}
</style>
