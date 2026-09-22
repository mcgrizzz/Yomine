<script lang="ts">
	import { untrack } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import Modal from './Modal.svelte';
	import {
		aboutModalOpen,
		checkForUpdate,
		installUpdate,
		openExternal,
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

	function openLink(event: MouseEvent) {
		event.preventDefault();
		openExternal((event.currentTarget as HTMLAnchorElement).href);
	}

	function close() {
		aboutModalOpen.set(false);
	}
</script>

<Modal open={$aboutModalOpen} title="About Yomine" width="min(440px, 92%)" flush onclose={close}>
	<div class="body">
		<section class="intro" aria-label="Application information">
			<div class="identity">
				<p class="name">Yomine</p>
				<div class="version">
					<span>v{version}</span>
					{#if __BUILD_COMMIT__}
						<span aria-hidden="true">·</span>
						<a
							class="commit"
							href={`${REPO}/commit/${__BUILD_COMMIT__}`}
							title={`View commit ${__BUILD_COMMIT__} on GitHub`}
							onclick={openLink}>{__BUILD_COMMIT__.slice(0, 7)}</a
						>
					{/if}
				</div>
			</div>
			<p class="tagline">Japanese vocabulary mining — 読み + mine.</p>
			<nav class="links" aria-label="Yomine links">
				<a href={REPO} onclick={openLink}>GitHub</a>
				<a href={`${REPO}/releases`} onclick={openLink}>Releases</a>
				<a href={`${REPO}/issues`} onclick={openLink}>Report an issue</a>
			</nav>
		</section>

		<div class="update-row">
			<div class="update-copy">
				<h3>Updates</h3>
				<div role="status">
					{#if $updateInfo}
						<p class="update-found">{$updateInfo.latest} is available</p>
					{:else if checkResult === 'up-to-date'}
						<p class="up-to-date">You're on the latest version</p>
					{:else if checkResult === 'unavailable'}
						<p class="unavailable">Couldn't reach GitHub — try again later</p>
					{/if}
				</div>
			</div>
			{#if $updateInfo}
				{@const u = $updateInfo}
				{#if u.installable}
					<button
						class="primary"
						title="Yomine restarts to finish installing; the loaded file and any queued mining are lost."
						onclick={() => (installArmed ? installUpdate() : (installArmed = true))}
					>
						{installArmed ? 'Restart & install now?' : 'Download & install'}
					</button>
				{:else}
					<button onclick={() => openExternal(u.url)}>Open release page</button>
				{/if}
			{:else}
				<button disabled={checking} onclick={runCheck}>
					{checking ? 'Checking…' : 'Check for updates'}
				</button>
			{/if}
		</div>

		<footer class="credits">
			<p>
				Kana matching includes adapted
				<a href="https://www.edrdg.org/edrdg/licence.html" onclick={openLink}>JMdict</a>
				data (EDRDG / James William Breen) and
				<a href="https://jitendex.org/pages/legal.html" onclick={openLink}>Jitendex</a>
				data (Stephen Kraus and contributors), under
				<a href="https://creativecommons.org/licenses/by-sa/4.0/" onclick={openLink}>CC BY-SA 4.0</a>.
			</p>
		</footer>
	</div>
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 1.1rem;
		padding: 1.25rem;
	}
	p {
		margin: 0;
	}
	a {
		color: var(--link);
		text-underline-offset: 0.2em;
		text-decoration-thickness: 1px;
	}
	a:hover {
		color: var(--accent);
	}
	.identity {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 0.4rem 1rem;
	}
	.name {
		font-size: 1.6rem;
		font-weight: 700;
		line-height: 1.2;
	}
	.version {
		display: flex;
		align-items: baseline;
		gap: 0.45rem;
		font-size: 0.8rem;
		color: var(--text-muted);
		white-space: nowrap;
	}
	.commit {
		font-family: monospace;
		font-size: 0.75rem;
	}
	.tagline {
		margin-top: 0.6rem;
		font-size: 0.85rem;
		line-height: 1.5;
		color: var(--text-muted);
	}
	.links {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 1rem;
		margin-top: 1rem;
		font-size: 0.8rem;
	}
	.update-row {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.75rem;
		padding-top: 1rem;
		border-top: 1px solid var(--border);
	}
	.update-copy {
		flex: 1 1 9rem;
		min-width: 0;
	}
	.update-copy h3 {
		margin: 0;
		font-size: 0.85rem;
		font-weight: 600;
	}
	.update-copy p {
		margin-top: 0.3rem;
		font-size: 0.78rem;
		line-height: 1.5;
		overflow-wrap: anywhere;
	}
	.update-row button {
		max-width: 100%;
		font-size: 0.8rem;
	}
	.update-found,
	.up-to-date {
		color: var(--success);
	}
	.unavailable {
		color: var(--text-muted);
	}
	.credits {
		padding-top: 1rem;
		border-top: 1px solid var(--border);
		font-size: 0.75rem;
		line-height: 1.6;
		color: var(--text-muted);
	}
	.credits a {
		color: inherit;
	}
	.credits a:hover {
		color: var(--text);
	}
</style>
