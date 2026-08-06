<script lang="ts">
	import { untrack } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import Modal from './Modal.svelte';
	import {
		aboutModalOpen,
		checkForUpdate,
		installUpdate,
		updateInfo,
		type UpdateCheckResult
	} from '$lib/stores';

	const REPO = 'https://github.com/mcgrizzz/Yomine';

	let version = $state('…');
	let checking = $state(false);
	let checkResult = $state<UpdateCheckResult | null>(null);
	let installArmed = $state(false);

	$effect(() => {
		if ($aboutModalOpen)
			untrack(() => {
				checkResult = null;
				installArmed = false;
				void getVersion().then((v) => (version = v));
			});
	});

	async function runCheck() {
		checking = true;
		checkResult = await checkForUpdate();
		checking = false;
	}

	function close() {
		aboutModalOpen.set(false);
	}
</script>

<Modal open={$aboutModalOpen} title="About Yomine" width="min(400px, 92%)" onclose={close}>
	<div class="body">
		<p class="name">Yomine <span class="version">v{version}</span></p>
		<p class="tagline">Japanese vocabulary mining — 読み + mine.</p>

		<div class="links">
			<button class="link" onclick={() => openUrl(REPO)}>GitHub</button>
			<button class="link" onclick={() => openUrl(`${REPO}/releases`)}>Releases</button>
			<button class="link" onclick={() => openUrl(`${REPO}/issues`)}>Report an issue</button>
		</div>

		<hr />

		<div class="update-row">
			{#if $updateInfo}
				{@const u = $updateInfo}
				<span class="update-found">{u.latest} is available</span>
				{#if u.installable}
					<button
						title="Yomine restarts to finish installing; the loaded file and any queued mining are lost."
						onclick={() => (installArmed ? installUpdate() : (installArmed = true))}
					>
						{installArmed ? 'Restart & install now?' : 'Download & install'}
					</button>
				{:else}
					<button onclick={() => openUrl(u.url)}>Open release page</button>
				{/if}
			{:else}
				<button disabled={checking} onclick={runCheck}>
					{checking ? 'Checking…' : 'Check for updates'}
				</button>
				{#if checkResult === 'up-to-date'}
					<span class="up-to-date">✓ You're on the latest version</span>
				{:else if checkResult === 'unavailable'}
					<span class="unavailable">Couldn't reach GitHub — try again later</span>
				{/if}
			{/if}
		</div>
	</div>
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.name {
		margin: 0;
		font-size: 1.3rem;
		font-weight: 700;
	}
	.version {
		font-size: 0.9rem;
		font-weight: 400;
		color: var(--text-muted);
	}
	.tagline {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.links {
		display: flex;
		gap: 0.4rem;
		margin-top: 0.25rem;
	}
	.link {
		padding: 0.2rem 0.6rem;
		font-size: 0.8rem;
	}
	hr {
		width: 100%;
		border: none;
		border-top: 1px solid var(--border);
		margin: 0.25rem 0;
	}
	.update-row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		min-height: 2rem;
		font-size: 0.85rem;
	}
	.update-found {
		color: var(--success);
		font-weight: 600;
	}
	.up-to-date {
		color: var(--success);
	}
	.unavailable {
		color: var(--text-muted);
	}
</style>
