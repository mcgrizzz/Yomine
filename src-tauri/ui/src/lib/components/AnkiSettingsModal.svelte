<script lang="ts">
	// Anki-settings modal (T040): parity with src/gui/settings/anki_settings_modal.rs
	// (+ its components.rs / anki_service.rs helpers). The known-interval and the
	// per-notetype field mappings are *staged* (egui's SettingsModalData) and
	// committed only on "Save Settings" via save_settings; Cancel reverts the staged
	// edits but keeps the modal open (egui behavior). Note types come from
	// list_anki_models; sample notes + the term/reading guesses are fetched lazily
	// per model via get_anki_sample_note (the guessing heuristic runs engine-side —
	// the same `anki::guess_field_mappings` egui calls).
	import { untrack } from 'svelte';
	import * as ipc from '$lib/ipc';
	import { ankiStatus, ankiModalOpen, settings, saveAnkiSettings } from '$lib/stores';

	/** `SettingsData::default().anki_interval` (core/settings.rs). */
	const DEFAULT_INTERVAL = 30;

	// ---- Staged settings (egui SettingsModalData) ----
	let tempMappings = $state<Record<string, ipc.FieldMapping>>({});
	let originalMappings = $state<Record<string, ipc.FieldMapping>>({});
	let tempInterval = $state(DEFAULT_INTERVAL);
	let originalInterval = $state(DEFAULT_INTERVAL);

	// ---- Note-type cache (egui available_models; persists across opens) ----
	let models = $state<ipc.AnkiModelInfo[]>([]);
	/** Per-model field guesses, cached alongside the sample note. */
	let guesses = $state<Record<string, { term: string | null; reading: string | null }>>({});
	let loadingModels = $state(false);
	let fetchError = $state<string | null>(null);

	// ---- Mapping editor (egui ModelMappingEditor) ----
	let edModel = $state('');
	let edTerm = $state('');
	let edReading = $state('');
	let edEditing = $state(false);
	let edOriginalName = $state<string | null>(null);

	// Hydrate the staged state each time the modal opens (egui open_settings).
	// untrack: hydrate writes state it also reads (tempInterval/tempMappings), so a
	// tracking read inside the effect body would loop (effect_update_depth_exceeded);
	// the effect must depend on the open flag only.
	$effect(() => {
		if ($ankiModalOpen) untrack(hydrate);
	});

	function hydrate() {
		const s = $settings;
		tempMappings = cloneMappings(s?.anki_model_mappings ?? {});
		originalMappings = cloneMappings(s?.anki_model_mappings ?? {});
		tempInterval = s?.anki_interval ?? DEFAULT_INTERVAL;
		originalInterval = tempInterval;
		resetEditor();
		// egui: fetch models on first open, then sample notes for mapped models so
		// the editor can show examples / guesses for them.
		if (models.length === 0) fetchModels();
		else fetchMappedSamples();
	}

	function cloneMappings(m: Record<string, ipc.FieldMapping>): Record<string, ipc.FieldMapping> {
		return Object.fromEntries(Object.entries(m).map(([k, v]) => [k, { ...v }]));
	}

	function mappingsEqual(
		a: Record<string, ipc.FieldMapping>,
		b: Record<string, ipc.FieldMapping>
	): boolean {
		const ka = Object.keys(a);
		const kb = Object.keys(b);
		if (ka.length !== kb.length) return false;
		return ka.every(
			(k) => b[k] && a[k].term_field === b[k].term_field && a[k].reading_field === b[k].reading_field
		);
	}

	const dirty = $derived(
		tempInterval !== originalInterval || !mappingsEqual(tempMappings, originalMappings)
	);

	// ---- Connection status (egui ui_connection_status, colored by content; the
	// resting Connected/offline state is live from the ambient `anki-status` event). ----
	const status = $derived.by(() => {
		if (loadingModels) return { text: 'Fetching models...', cls: 'pending' };
		if (fetchError) return { text: `Error: ${fetchError}`, cls: 'error' };
		if ($ankiStatus.connected) return { text: 'Connected', cls: 'ok' };
		return { text: 'Ready', cls: 'pending' };
	});

	async function fetchModels() {
		if (loadingModels) return;
		loadingModels = true;
		try {
			models = await ipc.listAnkiModels();
			fetchError = null;
		} catch (err) {
			// egui shows fetch failures inline on the status line (red "Error: …"),
			// not as a banner.
			fetchError = String(err);
		} finally {
			loadingModels = false;
		}
		if (!fetchError) await fetchMappedSamples();
	}

	/** Fetch sample notes for already-mapped models that lack one (egui open_settings). */
	async function fetchMappedSamples() {
		for (const name of Object.keys(tempMappings)) {
			const m = models.find((m) => m.name === name);
			if (m && !m.sample_note) await fetchSample(name);
		}
	}

	/** Fetch one model's sample note + engine-side field guesses, then auto-fill
	 * empty editor fields (egui fetch_sample_note → trigger_field_guessing). */
	async function fetchSample(name: string) {
		const model = models.find((m) => m.name === name);
		if (!model) return;
		const res = await ipc.getAnkiSampleNote(name, model.fields);
		model.sample_note = res.sample_note;
		guesses[name] = { term: res.guessed_term, reading: res.guessed_reading };
		if (edModel === name && !edTerm && !edReading) applyGuess(name);
	}

	function applyGuess(name: string) {
		const g = guesses[name];
		if (!g) return;
		if (g.term) edTerm = g.term;
		if (g.reading) edReading = g.reading;
	}

	const selectedModel = $derived(models.find((m) => m.name === edModel));

	// Selecting a note type clears the fields, then guesses from the (possibly
	// fetched) sample note (egui ui_model_selection).
	function onModelSelect() {
		edTerm = '';
		edReading = '';
		if (!edModel) return;
		const m = models.find((m) => m.name === edModel);
		if (m?.sample_note) applyGuess(edModel);
		else fetchSample(edModel);
	}

	function editMapping(name: string) {
		const mapping = tempMappings[name];
		if (!mapping) return;
		edModel = name;
		edTerm = mapping.term_field;
		edReading = mapping.reading_field;
		edEditing = true;
		edOriginalName = name;
	}

	function deleteMapping(name: string) {
		tempMappings = Object.fromEntries(Object.entries(tempMappings).filter(([k]) => k !== name));
	}

	function addOrUpdate() {
		if (!edModel || !edTerm || !edReading) return;
		const next = { ...tempMappings };
		if (edOriginalName && edOriginalName !== edModel) delete next[edOriginalName];
		next[edModel] = { term_field: edTerm, reading_field: edReading };
		tempMappings = next;
		resetEditor();
	}

	function resetEditor() {
		edModel = '';
		edTerm = '';
		edReading = '';
		edEditing = false;
		edOriginalName = null;
	}

	// DragValue parity: the interval clamps to 1..=365 instead of erroring.
	function clampInterval() {
		tempInterval = Math.min(365, Math.max(1, Math.round(tempInterval || DEFAULT_INTERVAL)));
	}

	function truncate(value: string): string {
		const chars = [...value];
		return chars.length > 30 ? chars.slice(0, 27).join('') + '...' : value;
	}

	async function save() {
		if (await saveAnkiSettings(cloneMappings(tempMappings), tempInterval)) {
			ankiModalOpen.set(false);
		}
		// On failure the lastError banner shows; staged state stays for a retry.
	}

	function cancel() {
		tempMappings = cloneMappings(originalMappings);
		tempInterval = originalInterval;
	}

	// egui resets the whole staged SettingsData here; scoped to the two fields
	// this modal owns so unrelated settings can't be clobbered by Save.
	function restoreDefault() {
		tempMappings = {};
		tempInterval = DEFAULT_INTERVAL;
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window onkeydown={(e) => $ankiModalOpen && e.key === 'Escape' && ankiModalOpen.set(false)} />

{#if $ankiModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={() => ankiModalOpen.set(false)}
		onkeydown={(e) => e.key === 'Escape' && ankiModalOpen.set(false)}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Anki settings"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Anki Settings</h2>
				<button class="close" aria-label="Close" onclick={() => ankiModalOpen.set(false)}
					>✕</button
				>
			</header>

			<div class="body">
				<!-- Known interval threshold (egui ui_known_interval_setting). -->
				<section>
					<h3>
						Known Interval Threshold
						<span
							class="info-icon"
							title="Cards with an interval at or above this threshold will be considered 'known' for comprehensibility estimation."
							>ℹ</span
						>
					</h3>
					<div class="row">
						<label for="anki-interval">Interval:</label>
						<input
							id="anki-interval"
							type="number"
							min="1"
							max="365"
							bind:value={tempInterval}
							onchange={clampInterval}
						/>
						<span class="hint">days (Default: 30 days)</span>
					</div>
				</section>

				<hr />

				<!-- Current notetypes (egui ui_existing_mappings). -->
				<section>
					<h3>Current Notetypes</h3>
					{#each Object.entries(tempMappings) as [name, mapping] (name)}
						<div class="row mapping-row">
							<span class="lbl">Notetype:</span>
							<strong class="model-name">{name}</strong>
							<span class="vsep"></span>
							<span class="lbl">Term Field:</span>
							<code class="model-name">{mapping.term_field}</code>
							<span class="vsep"></span>
							<span class="lbl">Reading Field:</span>
							<code class="model-name">{mapping.reading_field}</code>
							<button onclick={() => editMapping(name)}>Edit</button>
							<button onclick={() => deleteMapping(name)}>Delete</button>
						</div>
					{/each}
				</section>

				<hr />

				<!-- Mapping editor (egui ui_mapping_editor). -->
				<section>
					<h3>{edEditing ? 'Edit Notetype' : 'Add Notetype'}</h3>

					<div class="row">
						<span class="lbl">Anki Connection Status:</span>
						{#if loadingModels}
							<span class="spinner" aria-label="Fetching models"></span>
						{/if}
						<span class="status {status.cls}">{status.text}</span>
						<button disabled={loadingModels} onclick={fetchModels}>
							{loadingModels ? 'Refreshing...' : 'Refresh Notetypes'}
						</button>
					</div>

					<div class="row">
						<label for="anki-model">Notetype:</label>
						<select id="anki-model" bind:value={edModel} onchange={onModelSelect}>
							<option value="" disabled hidden></option>
							{#if edModel && !models.some((m) => m.name === edModel)}
								<option value={edModel}>{edModel}</option>
							{/if}
							{#each models as model (model.name)}
								<option value={model.name}>{model.name}</option>
							{/each}
						</select>
					</div>

					{#if selectedModel}
						{@const termExample = edTerm ? selectedModel.sample_note?.[edTerm] : undefined}
						{@const readingExample = edReading
							? selectedModel.sample_note?.[edReading]
							: undefined}
						<div class="row">
							<label for="anki-term-field">Term Field:</label>
							{#if edTerm && selectedModel.sample_note}
								<span class="guessed" title="This field was guessed based on its content">＊</span>
							{/if}
							<select id="anki-term-field" bind:value={edTerm}>
								<option value="" disabled hidden></option>
								{#each selectedModel.fields as f (f)}
									<option value={f}>{f}</option>
								{/each}
							</select>
							{#if termExample !== undefined}
								<span class="vsep"></span>
								<span class="lbl">Example:</span>
								<span class="example">"{truncate(termExample)}"</span>
							{/if}
						</div>
						<div class="row">
							<label for="anki-reading-field">Reading Field:</label>
							{#if edReading && selectedModel.sample_note}
								<span class="guessed" title="This field was guessed based on its content">＊</span>
							{/if}
							<select id="anki-reading-field" bind:value={edReading}>
								<option value="" disabled hidden></option>
								{#each selectedModel.fields as f (f)}
									<option value={f}>{f}</option>
								{/each}
							</select>
							{#if readingExample !== undefined}
								<span class="vsep"></span>
								<span class="lbl">Example:</span>
								<span class="example reading">"{truncate(readingExample)}"</span>
							{/if}
						</div>
					{/if}

					<div class="row">
						<button disabled={!edModel || !edTerm || !edReading} onclick={addOrUpdate}>
							{edEditing ? 'Update' : 'Add'}
						</button>
					</div>
				</section>
			</div>

			<hr />

			<div class="dirty">
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
		width: min(720px, 94vw);
		max-height: 86vh;
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
	.body {
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		padding: 0 1rem;
	}
	section {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
	}
	h3 {
		margin: 0;
		font-size: 0.95rem;
	}
	.info-icon {
		font-size: 0.75rem;
		color: var(--comment);
		cursor: help;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}
	.mapping-row {
		padding: 0.25rem 0.4rem;
		background: var(--bg-light);
		border-radius: var(--radius);
	}
	.lbl {
		font-size: 0.85rem;
	}
	.vsep {
		width: 1px;
		align-self: stretch;
		background: var(--border);
	}
	.model-name {
		color: var(--blue);
	}
	code.model-name {
		font-family: monospace;
	}
	.guessed {
		color: var(--cyan);
		cursor: help;
	}
	.status.ok {
		color: var(--green);
	}
	.status.error {
		color: var(--red);
	}
	.status.pending {
		color: var(--yellow);
	}
	.example {
		color: var(--green);
	}
	.example.reading {
		color: var(--blue);
	}
	input[type='number'] {
		width: 5.5rem;
		padding: 0.3rem 0.5rem;
		background: var(--bg-light);
		color: var(--fg);
		border: 1px solid var(--border);
		border-radius: 3px;
	}
	select {
		min-width: 11rem;
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
	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 0 1rem;
	}
	.body hr {
		margin: 0;
	}
	.dirty {
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
	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid var(--comment);
		border-top-color: var(--cyan);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
