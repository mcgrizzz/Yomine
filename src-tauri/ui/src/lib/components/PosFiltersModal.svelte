<script lang="ts">
	// Staged edits; Save both persists the defaults AND applies them to the live
	// table. Cancel reverts but keeps the modal open (egui behavior).
	import { untrack } from 'svelte';
	import { posCatalog, posEnabled, posModalOpen, savePosFilters } from '$lib/stores';

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

	/** egui `default_pos_map`: everything on except the low-value categories. */
	const DEFAULT_OFF = new Set(['Unknown', 'Other', 'Symbol', 'KanaExpression']);

	let staged = $state<Record<string, boolean>>({});
	let original = $state<Record<string, boolean>>({});

	// Seeds from the *live* table state, not the saved defaults. untrack: a
	// tracked read would clobber staged edits while open.
	$effect(() => {
		if ($posModalOpen) untrack(hydrate);
	});

	function hydrate() {
		const current: Record<string, boolean> = {};
		for (const p of $posCatalog) current[p.key] = $posEnabled[p.key] !== false;
		staged = current;
		original = { ...current };
	}

	const labelOf = $derived(
		new Map($posCatalog.map((p) => [p.key, p.display_name] as [string, string]))
	);
	const nounOn = $derived(staged['Noun'] !== false);
	const dirty = $derived($posCatalog.some((p) => staged[p.key] !== original[p.key]));

	function toggle(key: string) {
		staged[key] = !(staged[key] !== false);
	}

	async function save() {
		if (await savePosFilters({ ...staged })) {
			original = { ...staged };
			posModalOpen.set(false);
		}
		// On failure the lastError banner shows; staged state stays for a retry.
	}

	function cancel() {
		staged = { ...original };
	}

	function restoreDefault() {
		const next: Record<string, boolean> = {};
		for (const p of $posCatalog) next[p.key] = !DEFAULT_OFF.has(p.key);
		staged = next;
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window onkeydown={(e) => $posModalOpen && e.key === 'Escape' && posModalOpen.set(false)} />

{#if $posModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => posModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && posModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Part of speech filters"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Part of Speech Filters</h2>
				<button class="close" aria-label="Close" onclick={() => posModalOpen.set(false)}>✕</button>
			</header>

			<div class="chips">
				<!-- Parent "Noun" chip; its sub-categories grey out when it's off. -->
				<button
					class="chip parent"
					class:on={nounOn}
					onclick={() => toggle('Noun')}
					aria-pressed={nounOn}>{labelOf.get('Noun') ?? 'Noun'}</button
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
		width: min(560px, 92%);
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
		background: var(--bg-light);
		color: var(--fg);
		border: 1px solid var(--border);
		border-radius: 18px;
		break-inside: avoid;
	}
	.chip.parent {
		font-weight: 700;
	}
	.chip.on {
		background: color-mix(in srgb, var(--cyan) 22%, var(--bg-light));
		border-color: var(--cyan);
		color: var(--cyan);
	}
	.chip:hover:not(:disabled) {
		outline: 1px solid color-mix(in srgb, var(--fg) 40%, transparent);
		outline-offset: 2px;
	}
	.chip:disabled {
		opacity: 0.55;
		color: var(--comment);
		cursor: default;
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
	footer button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
