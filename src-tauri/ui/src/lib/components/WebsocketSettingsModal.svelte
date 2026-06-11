<script lang="ts">
	// WebSocket-settings modal (T041): parity with src/gui/settings/websocket_settings_modal.rs.
	// The port edit is *staged* (egui's temp_websocket_settings) and committed only on
	// "Save Settings" via set_websocket_port (persist + restart the running server);
	// Cancel reverts the staged edit but keeps the modal open (egui behavior).
	import { settings, websocketModalOpen, saveWebsocketPort } from '$lib/stores';

	/** `WebSocketSettings::default()` (core/settings.rs). */
	const DEFAULT_PORT = 8766;

	let tempPort = $state(DEFAULT_PORT);
	let originalPort = $state(DEFAULT_PORT);
	let status = $state<string | null>(null);

	// Hydrate from the settings mirror each time the modal opens (egui open_settings).
	$effect(() => {
		if ($websocketModalOpen) hydrate();
	});

	function hydrate() {
		const port = $settings?.websocket_settings.port ?? DEFAULT_PORT;
		tempPort = port;
		originalPort = port;
		status = null;
	}

	const dirty = $derived(tempPort !== originalPort);
	// egui's is_valid_port (>= 1024; u16 caps at 65535 — the number input doesn't).
	const valid = $derived(Number.isInteger(tempPort) && tempPort >= 1024 && tempPort <= 65535);

	async function save() {
		if (!valid) {
			status = 'Invalid port range. Please use ports 1024-65535.';
			return;
		}
		if (await saveWebsocketPort(tempPort)) {
			websocketModalOpen.set(false);
		}
		// On failure the lastError banner shows; staged state stays for a retry.
	}

	function cancel() {
		tempPort = originalPort;
		status = null;
	}

	function restoreDefault() {
		tempPort = DEFAULT_PORT;
		status = null;
	}
</script>

{#if $websocketModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => websocketModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && websocketModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="WebSocket server settings"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>WebSocket Server Settings</h2>
				<button class="close" aria-label="Close" onclick={() => websocketModalOpen.set(false)}
					>✕</button
				>
			</header>

			<!-- Port configuration (egui ui_port_configuration). -->
			<div class="port-row">
				<label for="ws-port">Server Port:</label>
				<input id="ws-port" type="number" min="1024" max="65535" bind:value={tempPort} />
				<span class="hint">(Valid range: 1024-65535)</span>
			</div>
			{#if !valid}
				<p class="invalid">⚠ Port must be between 1024 and 65535</p>
			{/if}

			{#if status}
				<p class="info">ℹ {status}</p>
			{/if}

			<hr />

			<div class="status">
				{#if dirty}⚠ Settings have been modified{/if}
			</div>

			<footer>
				<button disabled={!dirty} onclick={save}>Save Settings</button>
				<button disabled={!dirty} onclick={cancel}>Cancel</button>
				<button class="right" onclick={restoreDefault}>Restore Default</button>
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
		gap: 0.6rem;
		width: min(420px, 92vw);
		padding-bottom: 0.75rem;
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
	.port-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.port-row input {
		width: 6.5rem;
		padding: 0.3rem 0.5rem;
		background: var(--bg-light);
		color: var(--fg);
		border: 1px solid var(--border);
		border-radius: 3px;
	}
	.hint {
		font-size: 0.85rem;
		color: var(--comment);
	}
	.invalid {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--red);
	}
	.info {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--cyan);
	}
	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 0 1rem;
	}
	.status {
		min-height: 1.2rem;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--yellow);
	}
	footer {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	footer .right {
		margin-left: auto;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
