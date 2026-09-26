<script lang="ts">
	// Staged edits; Save both persists the defaults AND applies them to the live
	// table. Cancel reverts but keeps the modal open (egui behavior).
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import {
		defaultSettings,
		posCatalog,
		posEnabled,
		posModalOpen,
		savePosFilters
	} from '$lib/stores';

	// NounExpression is intentionally absent (hidden but still saved).
	const NOUN_CHILDREN = ['ProperNoun', 'CompoundNoun', 'AdjectivalNoun', 'SuruVerb'];
	const OTHER_CHIPS = [
		'Interjection',
		'Onomatopoeia',
		'Number',
		'Counter',
		'Verb',
		'Copula',
		'Adjective',
		'Preposition',
		'Postposition',
		'Prefix',
		'Suffix',
		'Pronoun',
		'Conjunction',
		'Adverb',
		'Determiner',
		'Symbol',
		'Expression',
		'KanaExpression',
		'Other',
		'Unknown'
	];

	// Seeds from the *live* table state, not the saved defaults.
	const form = settingsDraft({
		open: posModalOpen,
		initial: {} as Record<string, boolean>,
		load: () => Object.fromEntries($posCatalog.map((p) => [p.key, $posEnabled[p.key] !== false]))
	});
	const staged = $derived(form.value);

	const labelOf = $derived(
		new Map($posCatalog.map((p) => [p.key, p.display_name] as [string, string]))
	);
	const nounOn = $derived(staged['Noun'] !== false);

	function toggle(key: string) {
		staged[key] = !(staged[key] !== false);
	}

	async function save() {
		if (await savePosFilters({ ...staged })) {
			form.commit();
			posModalOpen.set(false);
		}
		// On failure the lastError banner shows; staged state stays for a retry.
	}

	function restoreDefault() {
		const off = $defaultSettings?.pos_filters;
		if (off) form.value = Object.fromEntries($posCatalog.map((p) => [p.key, off[p.key] !== false]));
	}
</script>

<Modal
	open={$posModalOpen}
	title="Part of Speech Filters"
	width="min(560px, 92%)"
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="chips">
		<!-- Parent "Noun" chip; its sub-categories grey out when it's off. -->
		<button class="chip parent" class:on={nounOn} onclick={() => toggle('Noun')} aria-pressed={nounOn}
			>{labelOf.get('Noun') ?? 'Noun'}</button
		>
		{#each NOUN_CHILDREN as key (key)}
			<button
				class="chip"
				class:on={staged[key] !== false}
				disabled={!nounOn}
				onclick={() => toggle(key)}
				aria-pressed={staged[key] !== false}>{labelOf.get(key) ?? key}</button
			>
		{/each}
		{#each OTHER_CHIPS as key (key)}
			<button
				class="chip"
				class:on={staged[key] !== false}
				onclick={() => toggle(key)}
				aria-pressed={staged[key] !== false}>{labelOf.get(key) ?? key}</button
			>
		{/each}
	</div>

	{#snippet footer()}
		<SettingsFooter {form} onsave={save} oncancel={form.revert} onrestore={restoreDefault} />
	{/snippet}
</Modal>

<style>
	/* egui flows chips top-to-bottom into width-based columns; CSS multi-column
	   gives the same vertical fill. */
	.chips {
		columns: 10rem;
		column-gap: 0.5rem;
		max-height: 520px;
		overflow-y: auto;
		padding: 0 1rem;
	}
	.chip {
		display: block;
		width: 100%;
		margin-bottom: 0.4rem;
		padding: 0.25rem 0.7rem;
		text-align: left;
		font-size: 0.85rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-pill);
		break-inside: avoid;
	}
	.chip.parent {
		font-weight: 700;
	}
	.chip.on {
		background: color-mix(in srgb, var(--accent) 22%, var(--bg-raised));
		border-color: var(--accent);
		color: var(--accent);
	}
	.chip:hover:not(:disabled) {
		outline: 1px solid color-mix(in srgb, var(--text) 40%, transparent);
		outline-offset: 2px;
	}
	.chip:disabled {
		opacity: 0.55;
		color: var(--text-muted);
		cursor: default;
	}
</style>
