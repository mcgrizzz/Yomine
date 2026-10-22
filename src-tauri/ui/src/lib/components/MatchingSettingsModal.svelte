<script lang="ts">
	import { untrack } from 'svelte';
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import { hasBuiltinTmdbKey } from '$lib/ipc';
	import {
		defaultSettings,
		matchingModalOpen,
		openExternal,
		saveShowMatching,
		settings
	} from '$lib/stores';

	let builtin = $state(true);
	void hasBuiltinTmdbKey().then((has) => (builtin = has));

	const form = settingsDraft({
		open: matchingModalOpen,
		initial: { key: '', matching: true },
		load: () =>
			$settings ? { key: $settings.tmdb_api_key, matching: $settings.tmdb_matching } : undefined
	});
	const draft = $derived(form.value);

	/** Starts open when a key of the user's own is saved. */
	let ownKey = $state(false);
	$effect(() => {
		if ($matchingModalOpen) untrack(() => (ownKey = !!$settings?.tmdb_api_key));
	});

	async function save() {
		if (!(await saveShowMatching(draft.key, draft.matching))) return;
		matchingModalOpen.set(false);
	}

	function restoreDefault() {
		const d = $defaultSettings;
		if (d) form.value = { key: d.tmdb_api_key, matching: d.tmdb_matching };
	}
</script>

<Modal
	open={$matchingModalOpen}
	title="Show matching"
	width="min(460px, 92%)"
	page
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<section aria-labelledby="matching-auto">
			<h4 id="matching-auto">Automatic matching</h4>
			<label class="check">
				<input type="checkbox" bind:checked={draft.matching} aria-describedby="matching-sent" />
				Automatically match shows and films
			</label>
			<p class="hint">
				Yomine reads the title from a file's name and looks it up on TMDB, to name its show or film
				and episode.
			</p>
			<p id="matching-sent" class="hint">
				Only that title, and a film's year, is sent to TMDB, not the whole file name or its folder.
				This covers files you open, earlier files not matched yet when Yomine starts, and other
				episodes found in the same folders. Searching for a match yourself sends what you type, even
				with this off.
			</p>
		</section>

		<section aria-labelledby="matching-access">
			<h4 id="matching-access">TMDB access</h4>
			<p class="hint">
				{#if $settings?.tmdb_api_key}
					Using your own API key.
				{:else if builtin}
					Using Yomine's built-in API key.
				{:else}
					This build has no built-in API key. Add your own to match shows and films.
				{/if}
			</p>
			<details bind:open={ownKey}>
				<summary>Use your own API key</summary>
				<div class="own-key">
					<label for="tmdb-key">API key</label>
					<input
						id="tmdb-key"
						type="password"
						autocomplete="off"
						spellcheck="false"
						aria-describedby="tmdb-key-help"
						bind:value={draft.key}
					/>
					<div id="tmdb-key-help" class="hint help">
						<p>
							<button
								type="button"
								class="link"
								onclick={() => openExternal('https://www.themoviedb.org/settings/api')}
								>Get a key on TMDB ↗</button
							>. Use the API Key, not the Read Access Token.
						</p>
						{#if builtin}
							<p>Clear the field and save to go back to the built-in key.</p>
						{/if}
					</div>
				</div>
			</details>
		</section>
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			onsave={save}
			oncancel={form.revert}
			restore={{
				label: 'Reset Show Matching Settings',
				title: 'Turn automatic matching back on and clear your own API key.',
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
	section {
		display: grid;
		justify-items: start;
		gap: 0.4rem;
	}
	h4 {
		margin: 0 0 0.2rem;
		font-size: 0.85rem;
		font-weight: 600;
	}
	p {
		margin: 0;
	}
	.hint {
		font-size: 0.85rem;
		line-height: 1.45;
		color: var(--text-muted);
	}
	.check {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	details {
		justify-self: stretch;
	}
	summary {
		cursor: pointer;
		font-size: 0.85rem;
	}
	.own-key {
		display: grid;
		grid-template-columns: 5rem minmax(0, 22rem);
		align-items: center;
		gap: 0.4rem 0.9rem;
		margin-top: 0.6rem;
		padding-left: 1rem;
	}
	.own-key .help {
		display: grid;
		gap: 0.2rem;
		grid-column: 2;
	}
	.link {
		padding: 0;
		border: none;
		background: transparent;
		color: var(--link);
		font-size: inherit;
	}
	.link:hover:not(:disabled) {
		background: transparent;
		text-decoration: underline;
	}
	@media (max-width: 640px) {
		.own-key {
			grid-template-columns: minmax(0, 1fr);
		}
		.own-key .help {
			grid-column: 1;
		}
	}
</style>
