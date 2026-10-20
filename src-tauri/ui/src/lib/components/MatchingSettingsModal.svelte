<script lang="ts">
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import { hasBuiltinTmdbKey } from '$lib/ipc';
	import { matchingModalOpen, openExternal, setTmdbApiKey, settings } from '$lib/stores';

	let builtin = $state(true);
	void hasBuiltinTmdbKey().then((has) => (builtin = has));

	const form = settingsDraft({
		open: matchingModalOpen,
		initial: { key: '' },
		load: () => ($settings ? { key: $settings.tmdb_api_key } : undefined)
	});
	const draft = $derived(form.value);

	async function save() {
		if (!(await setTmdbApiKey(draft.key))) return;
		matchingModalOpen.set(false);
	}
</script>

<Modal
	open={$matchingModalOpen}
	title="Show Matching"
	width="min(460px, 92%)"
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<p class="status">
			{#if $settings?.tmdb_api_key}
				<span class="ok">●</span> Using your TMDB key
			{:else if builtin}
				<span class="ok">●</span> Using Yomine's built-in TMDB key
			{:else}
				<span class="off">●</span> No TMDB key: files aren't matched
			{/if}
		</p>
		<label for="tmdb-key">Your own key <span class="muted">(optional)</span></label>
		<input
			id="tmdb-key"
			type="password"
			autocomplete="off"
			spellcheck="false"
			placeholder={builtin ? 'Leave empty to use the built-in key' : 'TMDB API key'}
			bind:value={draft.key}
		/>
		<p class="muted">
			<button class="link" onclick={() => openExternal('https://www.themoviedb.org/settings/api')}
				>Get a key ↗</button
			> · use the API Key, not the Read Access Token
		</p>
	</div>

	{#snippet footer()}
		<SettingsFooter {form} onsave={save} oncancel={form.revert} />
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	p {
		margin: 0;
	}
	.muted {
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.ok {
		color: var(--status-ok);
	}
	.off {
		color: var(--status-off);
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
</style>
