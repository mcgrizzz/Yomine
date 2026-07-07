<script lang="ts">
	// Weight edits are staged (Cancel reverts but stays open — egui behavior);
	// install/update/import/remove are immediate and re-hydrate the list,
	// resetting staged edits with it.
	import { untrack } from 'svelte';
	import {
		settings,
		frequencyModalOpen,
		saveDictionaryStates,
		recommendedDicts
	} from '$lib/stores';
	import * as ipc from '$lib/ipc';

	const MIN_WEIGHT = 0.1;
	const MAX_WEIGHT = 5.0;

	let entries = $state<ipc.DictionaryState[]>([]);
	let original = $state<ipc.DictionaryState[]>([]);
	let loaded = $state(false);

	// The recommended catalog is checked at launch + the manual button only (it
	// hits the network). `busyTitle` serializes every mutating action.
	let checking = $state(false);
	let opError = $state<string | null>(null);
	let busyTitle = $state<string | null>(null);
	let busyMsg = $state<string | null>(null);
	let confirmRemove = $state<string | null>(null);

	// Hydrate each time the modal opens.
	// untrack: hydrate reads $settings, which must not become a dependency
	// (a tracked read would re-hydrate and clobber staged edits while open).
	$effect(() => {
		if ($frequencyModalOpen)
			untrack(() => {
				confirmRemove = null;
				opError = null;
				void hydrate();
			});
	});

	async function checkUpdates() {
		checking = true;
		opError = null;
		try {
			recommendedDicts.set(await ipc.getRecommendedDictionaries());
		} catch (err) {
			opError = String(err);
		} finally {
			checking = false;
		}
	}

	async function importFromFile() {
		busyTitle = '(import)';
		opError = null;
		try {
			const copied = await ipc.loadFrequencyDictionaries((m) => (busyMsg = m.message));
			if (copied > 0) {
				await hydrate();
				await checkUpdates(); // imported zips can change recommended install states
			}
		} catch (err) {
			opError = String(err);
		} finally {
			busyTitle = null;
			busyMsg = null;
		}
	}

	const badgeText: Record<ipc.RecommendedDictionary['status'], string> = {
		'not-installed': 'Not installed',
		installed: 'Installed',
		'up-to-date': 'Up to date',
		'update-available': 'Update available'
	};

	async function install(title: string) {
		busyTitle = title;
		opError = null;
		try {
			await ipc.installRecommendedDictionary(title, (m) => (busyMsg = m.message));
			await hydrate(); // the list changed; staged edits reset with it
			await checkUpdates();
		} catch (err) {
			opError = String(err);
		} finally {
			busyTitle = null;
			busyMsg = null;
		}
	}

	async function removeDict(name: string) {
		confirmRemove = null;
		busyTitle = name;
		opError = null;
		try {
			await ipc.removeDictionary(name, (m) => (busyMsg = m.message));
			// Mirror the dropped weight entry into the local settings store.
			const s = $settings;
			if (s) {
				const frequency_weights = { ...s.frequency_weights };
				delete frequency_weights[name];
				settings.set({ ...s, frequency_weights });
			}
			await hydrate();
			await checkUpdates();
		} catch (err) {
			opError = String(err);
		} finally {
			busyTitle = null;
			busyMsg = null;
		}
	}

	async function hydrate() {
		loaded = false;
		entries = [];
		original = [];
		const dicts = await ipc.listDictionaries();
		const weights = $settings?.frequency_weights ?? {};
		let list: ipc.DictionaryState[];
		if (dicts.length > 0) {
			// Live manager states, with any persisted setting taking precedence.
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

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window
	onkeydown={(e) => $frequencyModalOpen && e.key === 'Escape' && frequencyModalOpen.set(false)}
/>

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
			aria-label="Frequency dictionaries"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Frequency Dictionaries</h2>
				<button class="close" aria-label="Close" onclick={() => frequencyModalOpen.set(false)}
					>✕</button
				>
			</header>

			<section class="recommended">
				<div class="rec-head">
					<h3>Recommended</h3>
					<button disabled={checking || busyTitle !== null} onclick={checkUpdates}>
						{checking ? 'Checking…' : '⟳ Check for updates'}
					</button>
				</div>
				{#if $recommendedDicts === null}
					<p class="hint">Updates not checked yet.</p>
				{:else}
					{#each $recommendedDicts as r (r.title)}
						<div class="rec-row">
							<div class="rec-text">
								<span class="rec-name">
									{r.name}
									{#if r.status === 'update-available'}
										<span class="rev">{r.installed_revision} → {r.latest_revision}</span>
									{:else if r.installed_revision ?? r.latest_revision}
										<span class="rev">({r.installed_revision ?? r.latest_revision})</span>
									{/if}
								</span>
								<span class="rec-desc">{r.description}</span>
							</div>
							<span class="badge {r.status}">{badgeText[r.status]}</span>
							{#if r.status === 'not-installed' || r.status === 'update-available'}
								<button disabled={busyTitle !== null} onclick={() => install(r.title)}>
									{busyTitle === r.title
										? 'Working…'
										: r.status === 'not-installed'
											? 'Download'
											: 'Update'}
								</button>
							{/if}
						</div>
					{/each}
				{/if}
				{#if busyMsg}<p class="hint">{busyMsg}</p>{/if}
				{#if opError}<p class="op-error">{opError}</p>{/if}
			</section>

			<hr />

			{#if loaded && entries.length === 0}
				<p class="empty">No frequency dictionaries loaded.</p>
			{:else if loaded}
				<div class="table">
					<div class="thead">
						<span></span>
						<span>Dictionary</span>
						<span>Weight</span>
						<span>Value</span>
						<span></span>
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
							<span class="del">
								{#if confirmRemove === entry.name}
									<button
										class="danger"
										disabled={busyTitle !== null}
										onclick={() => removeDict(entry.name)}>Confirm</button
									>
									<button aria-label="Keep dictionary" onclick={() => (confirmRemove = null)}
										>✕</button
									>
								{:else}
									<button
										class="ghost"
										title={`Remove ${entry.name}`}
										disabled={busyTitle !== null}
										onclick={() => (confirmRemove = entry.name)}>🗑</button
									>
								{/if}
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
				<button class="right" disabled={busyTitle !== null} onclick={importFromFile}>
					{busyTitle === '(import)' ? 'Importing…' : 'Import from file…'}
				</button>
				<button onclick={restoreDefault}>Restore Default</button>
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
	/* Recommended section. */
	.recommended {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0 1rem;
	}
	.rec-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.rec-head button {
		font-size: 0.75rem;
		padding: 0.2rem 0.5rem;
	}
	.recommended h3 {
		margin: 0;
		font-size: 0.85rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--comment);
	}
	.rec-row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}
	.rec-text {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}
	.rec-name {
		font-weight: 600;
	}
	.rev {
		font-weight: 400;
		font-size: 0.8rem;
		color: var(--comment);
	}
	.rec-desc {
		font-size: 0.8rem;
		color: var(--comment);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.badge {
		font-size: 0.75rem;
		padding: 0.15rem 0.5rem;
		border-radius: 999px;
		border: 1px solid var(--border);
		white-space: nowrap;
	}
	.badge.up-to-date {
		color: var(--green);
		border-color: var(--green);
	}
	.badge.update-available {
		color: var(--yellow);
		border-color: var(--yellow);
	}
	.badge.not-installed,
	.badge.installed {
		color: var(--comment);
	}
	.hint {
		margin: 0;
		font-size: 0.8rem;
		color: var(--comment);
	}
	.op-error {
		margin: 0;
		font-size: 0.8rem;
		color: var(--red);
	}
	/* Per-row remove (two-step confirm). */
	.del {
		display: inline-flex;
		justify-content: flex-end;
		gap: 0.25rem;
	}
	.del .ghost {
		padding: 0.1rem 0.3rem;
		background: transparent;
		border: none;
		opacity: 0.6;
	}
	.del .ghost:hover:not(:disabled) {
		opacity: 1;
	}
	.del .danger {
		padding: 0.15rem 0.4rem;
		font-size: 0.75rem;
		color: #fff;
		background: var(--red);
		border: none;
		border-radius: 3px;
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
		grid-template-columns: 1.4rem minmax(9rem, auto) 1fr 5.6rem minmax(2rem, auto);
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
