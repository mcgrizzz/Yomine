<script lang="ts">
	import { untrack } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { openUrl } from '@tauri-apps/plugin-opener';
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

	$effect(() => {
		if ($aboutModalOpen)
			untrack(() => {
				checkResult = null;
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

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window onkeydown={(e) => $aboutModalOpen && e.key === 'Escape' && close()} />

{#if $aboutModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={close}
		onkeydown={(e) => e.key === 'Escape' && close()}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="About Yomine"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>About Yomine</h2>
				<button class="close" aria-label="Close" onclick={close}>✕</button>
			</header>

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
							<button onclick={installUpdate}>Download &amp; install</button>
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
		width: min(400px, 92vw);
		padding-bottom: 1rem;
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
		color: var(--comment);
	}
	.tagline {
		margin: 0;
		font-size: 0.85rem;
		color: var(--comment);
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
		color: var(--green);
		font-weight: 600;
	}
	.up-to-date {
		color: var(--green);
	}
	.unavailable {
		color: var(--comment);
	}
</style>
