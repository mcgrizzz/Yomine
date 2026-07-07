<script lang="ts">
	// Staged edits; Cancel reverts but keeps the modal open (egui behavior).
	// Saving the port also restarts a running server on it.
	import { untrack } from 'svelte';
	import {
		settings,
		websocketModalOpen,
		saveWebsocketPort,
		setAsbplayerPollSecs
	} from '$lib/stores';

	/** `WebSocketSettings::default()` (core/settings.rs). */
	const DEFAULT_PORT = 8766;
	/** `default_asbplayer_poll_secs()` (core/settings.rs). */
	const DEFAULT_POLL_SECS = 3;

	let tempPort = $state(DEFAULT_PORT);
	let originalPort = $state(DEFAULT_PORT);
	let tempPoll = $state(DEFAULT_POLL_SECS);
	let originalPoll = $state(DEFAULT_POLL_SECS);
	let status = $state<string | null>(null);

	// untrack: a tracked $settings read would re-hydrate (clobbering the staged
	// edit) on any settings change while open.
	$effect(() => {
		if ($websocketModalOpen) untrack(hydrate);
	});

	function hydrate() {
		const port = $settings?.websocket_settings.port ?? DEFAULT_PORT;
		tempPort = port;
		originalPort = port;
		const poll = $settings?.asbplayer_poll_secs ?? DEFAULT_POLL_SECS;
		tempPoll = poll;
		originalPoll = poll;
		status = null;
	}

	const dirty = $derived(tempPort !== originalPort || tempPoll !== originalPoll);
	// u16 caps at 65535 — the number input doesn't.
	const valid = $derived(Number.isInteger(tempPort) && tempPort >= 1024 && tempPort <= 65535);
	const pollValid = $derived(Number.isInteger(tempPoll) && tempPoll >= 1 && tempPoll <= 60);

	async function save() {
		if (!valid) {
			status = 'Invalid port range. Please use ports 1024-65535.';
			return;
		}
		if (!pollValid) {
			status = 'Poll interval must be 1-60 seconds.';
			return;
		}
		if (tempPoll !== originalPoll) {
			await setAsbplayerPollSecs(tempPoll);
			originalPoll = tempPoll;
		}
		if (tempPort !== originalPort) {
			if (!(await saveWebsocketPort(tempPort))) return;
			// On failure the lastError banner shows; staged state stays for a retry.
		}
		websocketModalOpen.set(false);
	}

	function cancel() {
		tempPort = originalPort;
		tempPoll = originalPoll;
		status = null;
	}

	function restoreDefault() {
		tempPort = DEFAULT_PORT;
		tempPoll = DEFAULT_POLL_SECS;
		status = null;
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window
	onkeydown={(e) => $websocketModalOpen && e.key === 'Escape' && websocketModalOpen.set(false)}
/>

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

			<div class="port-row">
				<label for="ws-port">Server Port:</label>
				<input id="ws-port" type="number" min="1024" max="65535" bind:value={tempPort} />
				<span class="hint">(Valid range: 1024-65535)</span>
			</div>
			{#if !valid}
				<p class="invalid">⚠ Port must be between 1024 and 65535</p>
			{/if}

			<div class="port-row">
				<label for="asb-poll">asbplayer poll interval:</label>
				<input id="asb-poll" type="number" min="1" max="60" bind:value={tempPoll} />
				<span class="hint">seconds (1-60; used by follow mode)</span>
			</div>
			{#if !pollValid}
				<p class="invalid">⚠ Poll interval must be between 1 and 60 seconds</p>
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
