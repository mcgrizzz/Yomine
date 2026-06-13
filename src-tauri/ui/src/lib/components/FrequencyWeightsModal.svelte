<script lang="ts">
	// Frequency-weights modal (T042): parity with src/gui/settings/frequency_weights_modal.rs.
	// One staged row per dictionary — enabled checkbox, name, a logarithmic 0.1–5.0
	// weight slider, and a numeric value (egui's DragValue, 2 decimals, "x" suffix).
	// Edits are staged (egui's `entries` vs `original`) and committed only on "Save
	// Settings" (per-dictionary `set_dictionary_state`); Cancel reverts the staged
	// edits but keeps the modal open (egui behavior).
	import { untrack } from 'svelte';
	import { settings, frequencyModalOpen, saveDictionaryStates } from '$lib/stores';
	import * as ipc from '$lib/ipc';

	const MIN_WEIGHT = 0.1;
	const MAX_WEIGHT = 5.0;

	let entries = $state<ipc.DictionaryState[]>([]);
	let original = $state<ipc.DictionaryState[]>([]);
	let loaded = $state(false);

	// Hydrate each time the modal opens (egui open_modal → build_entries).
	// untrack: hydrate reads $settings, which must not become a dependency
	// (a tracked read would re-hydrate and clobber staged edits while open).
	$effect(() => {
		if ($frequencyModalOpen) untrack(() => void hydrate());
	});

	async function hydrate() {
		loaded = false;
		entries = [];
		original = [];
		const dicts = await ipc.listDictionaries();
		const weights = $settings?.frequency_weights ?? {};
		let list: ipc.DictionaryState[];
		if (dicts.length > 0) {
			// Live manager states, with any persisted setting taking precedence
			// (egui build_entries).
			list = dicts.map((d) => ({
				name: d.name,
				weight: Math.max(weights[d.name]?.weight ?? d.weight, MIN_WEIGHT),
				enabled: weights[d.name]?.enabled ?? d.enabled
			}));
		} else {
			// Tools not loaded yet: fall back to the persisted settings (egui parity).
			list = Object.entries(weights)
				.map(([name, w]) => ({ name, weight: Math.max(w.weight, MIN_WEIGHT), enabled: w.enabled }))
				.sort((a, b) => a.name.localeCompare(b.name));
		}
		entries = list;
		original = list.map((e) => ({ ...e }));
		loaded = true;
	}

	const dirty = $derived(
		entries.some(
			(e, i) => e.weight !== original[i]?.weight || e.enabled !== original[i]?.enabled
		)
	);

	// egui's Slider is logarithmic over 0.1..=5.0; map it onto a linear 0..1000 range.
	const SLIDER_STEPS = 1000;
	const LOG_SPAN = Math.log(MAX_WEIGHT / MIN_WEIGHT);
	const toSlider = (w: number) => Math.round((Math.log(w / MIN_WEIGHT) / LOG_SPAN) * SLIDER_STEPS);
	const fromSlider = (t: number) => clampWeight(MIN_WEIGHT * Math.exp((t / SLIDER_STEPS) * LOG_SPAN));

	function clampWeight(w: number): number {
		if (!Number.isFinite(w)) return MIN_WEIGHT;
		return Math.round(Math.min(Math.max(w, MIN_WEIGHT), MAX_WEIGHT) * 100) / 100;
	}

	function setEnabled(i: number, on: boolean) {
		entries[i].enabled = on;
		if (!on) entries[i].weight = Math.max(entries[i].weight, MIN_WEIGHT);
	}

	async function save() {
		// egui saves the whole map; the per-dictionary command makes the changed
		// subset the minimal equivalent commit.
		const changed = entries.filter(
			(e, i) => e.weight !== original[i]?.weight || e.enabled !== original[i]?.enabled
		);
		if (await saveDictionaryStates(changed.map((e) => ({ ...e })))) {
			original = entries.map((e) => ({ ...e }));
			frequencyModalOpen.set(false);
		}
		// On failure the lastError banner shows; staged state stays for a retry.
	}

	function cancel() {
		entries = original.map((e) => ({ ...e }));
	}

	function restoreDefault() {
		for (const e of entries) {
			e.enabled = true;
			e.weight = 1.0;
		}
	}
</script>

{#if $frequencyModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => frequencyModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && frequencyModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Frequency dictionary weights"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Frequency Dictionary Weights</h2>
				<button class="close" aria-label="Close" onclick={() => frequencyModalOpen.set(false)}
					>✕</button
				>
			</header>

			{#if loaded && entries.length === 0}
				<p class="empty">No frequency dictionaries loaded.</p>
			{:else if loaded}
				<div class="table">
					<div class="thead">
						<span></span>
						<span>Dictionary</span>
						<span>Weight</span>
						<span>Value</span>
					</div>
					{#each entries as entry, i (entry.name)}
						<div class="row">
							<input
								type="checkbox"
								checked={entry.enabled}
								onchange={(e) => setEnabled(i, e.currentTarget.checked)}
								aria-label={`Enable ${entry.name}`}
							/>
							<span class="name" class:off={!entry.enabled}>{entry.name}</span>
							<input
								type="range"
								min="0"
								max={SLIDER_STEPS}
								value={toSlider(entry.weight)}
								disabled={!entry.enabled}
								oninput={(e) => (entry.weight = fromSlider(e.currentTarget.valueAsNumber))}
								aria-label={`${entry.name} weight`}
							/>
							<span class="value">
								<input
									type="number"
									min={MIN_WEIGHT}
									max={MAX_WEIGHT}
									step="0.05"
									value={entry.weight}
									disabled={!entry.enabled}
									onchange={(e) => (entry.weight = clampWeight(e.currentTarget.valueAsNumber))}
								/>x
							</span>
						</div>
					{/each}
				</div>
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
		width: min(620px, 92vw);
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
	.empty {
		margin: 0;
		padding: 0 1rem;
		color: var(--comment);
	}
	.table {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		max-height: 260px;
		overflow-y: auto;
		padding: 0 1rem;
	}
	.thead,
	.row {
		display: grid;
		grid-template-columns: 1.4rem minmax(10rem, auto) 1fr 5.6rem;
		align-items: center;
		gap: 0.5rem;
	}
	.thead {
		font-weight: 600;
		font-size: 0.85rem;
		padding-bottom: 0.2rem;
		border-bottom: 1px solid var(--border);
	}
	.row:nth-child(even) {
		background: color-mix(in srgb, var(--bg-light) 45%, transparent);
	}
	.name {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.name.off {
		color: var(--comment);
	}
	.row input[type='range'] {
		width: 100%;
	}
	.value {
		display: flex;
		align-items: center;
		gap: 0.15rem;
		font-variant-numeric: tabular-nums;
	}
	.value input {
		width: 4.4rem;
		padding: 0.2rem 0.35rem;
		background: var(--bg-light);
		color: var(--fg);
		border: 1px solid var(--border);
		border-radius: 3px;
	}
	input:disabled {
		opacity: 0.5;
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
