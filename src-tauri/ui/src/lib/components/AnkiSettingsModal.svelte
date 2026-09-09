<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { dirtyGuard } from '$lib/dirtyGuard.svelte';
	import Modal from './Modal.svelte';
	import * as ipc from '$lib/ipc';
	import { ankiModalOpen, settings, saveAnkiSettings } from '$lib/stores';

	type Draft = Pick<
		ipc.SettingsData,
		'anki_connection' | 'anki_model_mappings' | 'anki_interval' | 'yomitan_url'
	>;
	type Phase = 'idle' | 'loading' | 'ready' | 'failed';
	const DEFAULT_PORT = 8765;
	const DEFAULT_INTERVAL = 30;
	const DEFAULT_YOMITAN_URL = 'http://127.0.0.1:19633';
	const defaults: Draft = {
		anki_connection: { port: DEFAULT_PORT, api_key: '' },
		anki_model_mappings: {},
		anki_interval: DEFAULT_INTERVAL,
		yomitan_url: DEFAULT_YOMITAN_URL
	};
	function copyDraft(s: Draft): Draft {
		return {
			anki_connection: { ...s.anki_connection },
			anki_model_mappings: Object.fromEntries(
				Object.entries(s.anki_model_mappings).map(([name, fields]) => [
					name,
					{ ...fields, sentence_field: fields.sentence_field ?? null }
				])
			),
			anki_interval: s.anki_interval,
			yomitan_url: s.yomitan_url
		};
	}
	let original = $state(copyDraft(defaults));
	let draft = $state(copyDraft(defaults));
	let ankiExpanded = $state(false);
	let yomitanExpanded = $state(false);
	let showKey = $state(false);
	let expandedModel = $state<string | null>(null);
	let adding = $state(false);
	let selectedNewModel = $state('');
	let removed = $state<{ name: string; mapping: ipc.FieldMapping } | null>(null);
	let saving = $state(false);
	let saveError = $state<string | null>(null);
	let ankiPhase = $state<Phase>('idle');
	let ankiError = $state<ipc.ConnectionError | null>(null);
	let yomitanPhase = $state<Phase>('idle');
	let yomitanError = $state<string | null>(null);
	let yomitanVersion = $state<string | null>(null);
	let catalogPhase = $state<Phase>('idle');
	let catalogError = $state<string | null>(null);
	let models = $state<ipc.AnkiModelInfo[]>([]);
	let samples = $state<Record<string, ipc.SampleNote>>({});
	let sampleErrors = $state<Record<string, string>>({});
	let catalogConnection = '';
	let ankiGeneration = 0;
	let yomitanGeneration = 0;

	function mappingsEqual(a: Draft['anki_model_mappings'], b: Draft['anki_model_mappings']) {
		return (
			Object.keys(a).length === Object.keys(b).length &&
			Object.entries(a).every(
				([name, fields]) =>
					b[name] &&
					fields.term_field === b[name].term_field &&
					fields.reading_field === b[name].reading_field &&
					(fields.sentence_field ?? null) === (b[name].sentence_field ?? null)
			)
		);
	}
	const dirty = $derived(
		draft.anki_connection.port !== original.anki_connection.port ||
			draft.anki_connection.api_key !== original.anki_connection.api_key ||
			draft.anki_interval !== original.anki_interval ||
			draft.yomitan_url !== original.yomitan_url ||
			!mappingsEqual(draft.anki_model_mappings, original.anki_model_mappings)
	);
	const validPort = $derived(
		Number.isInteger(draft.anki_connection.port) &&
			draft.anki_connection.port >= 1 &&
			draft.anki_connection.port <= 65535
	);
	const availableModels = $derived(
		models.filter((model) => !draft.anki_model_mappings[model.name])
	);
	const guard = dirtyGuard(
		() => dirty,
		() => ankiModalOpen.set(false)
	);

	$effect(() => {
		if (!$ankiModalOpen) return;
		untrack(hydrate);
		return () => {
			ankiGeneration++;
			yomitanGeneration++;
		};
	});

	function hydrate() {
		original = copyDraft($settings ?? defaults);
		revert();
	}

	function revert() {
		draft = copyDraft(original);
		ankiGeneration++;
		yomitanGeneration++;
		if (catalogConnection !== JSON.stringify(draft.anki_connection)) clearCatalog();
		else if (catalogPhase === 'loading') catalogPhase = 'idle';
		expandedModel = null;
		adding = false;
		selectedNewModel = '';
		removed = null;
		showKey = false;
		saveError = null;
		guard.disarm();
		void testAnki();
		void testYomitan();
	}

	function clearCatalog() {
		models = [];
		samples = {};
		sampleErrors = {};
		catalogPhase = 'idle';
		catalogError = null;
		catalogConnection = '';
	}

	function restoreDefault() {
		draft = copyDraft(defaults);
		expandedModel = null;
		adding = false;
		selectedNewModel = '';
		removed = null;
		showKey = false;
		saveError = null;
		editConnection();
		editYomitan();
		guard.disarm();
	}

	function editConnection() {
		ankiGeneration++;
		ankiPhase = 'idle';
		ankiError = null;
		clearCatalog();
	}

	function editYomitan() {
		yomitanGeneration++;
		yomitanPhase = 'idle';
		yomitanError = null;
		yomitanVersion = null;
	}

	async function testAnki() {
		if (!validPort) return;
		const generation = ankiGeneration;
		ankiPhase = 'loading';
		ankiError = null;
		try {
			await ipc.testAnkiConnection({ ...draft.anki_connection });
			if (generation !== ankiGeneration) return;
			ankiPhase = 'ready';
			if (catalogPhase !== 'ready') await fetchModels();
		} catch (error) {
			if (generation !== ankiGeneration) return;
			ankiPhase = 'failed';
			ankiError =
				typeof error === 'object' && error !== null && 'message' in error && 'detail' in error
					? (error as ipc.ConnectionError)
					: {
							message: 'Connection test failed. Check the Anki add-on settings.',
							detail: String(error)
						};
		}
	}

	async function fetchModels() {
		if (catalogPhase === 'loading') return;
		const generation = ankiGeneration;
		const connection = { ...draft.anki_connection };
		catalogPhase = 'loading';
		catalogError = null;
		try {
			const result = await ipc.listAnkiModels(connection);
			if (generation !== ankiGeneration) return;
			models = result;
			catalogConnection = JSON.stringify(connection);
			catalogPhase = 'ready';
		} catch (error) {
			if (generation !== ankiGeneration) return;
			catalogPhase = 'failed';
			catalogError = String(error);
		}
	}

	function validUrl(url: string) {
		try {
			return ['http:', 'https:'].includes(new URL(url).protocol);
		} catch {
			return false;
		}
	}

	async function testYomitan() {
		const url = draft.yomitan_url.trim();
		if (!validUrl(url)) return;
		const generation = yomitanGeneration;
		yomitanPhase = 'loading';
		yomitanError = null;
		try {
			const result = await ipc.getYomitanStatus(url);
			if (generation !== yomitanGeneration) return;
			yomitanPhase = result.reachable ? 'ready' : 'failed';
			yomitanVersion = result.version;
			if (!result.reachable)
				yomitanError =
					'Cannot reach Yomitan API. Check that the companion is running and the URL is correct.';
		} catch (error) {
			if (generation !== yomitanGeneration) return;
			yomitanPhase = 'failed';
			yomitanError = String(error);
		}
	}

	async function loadSample(name: string, suggest = false) {
		const model = models.find((m) => m.name === name);
		const mapping = draft.anki_model_mappings[name];
		if (!model || !mapping) return;
		const generation = ankiGeneration;
		const before = JSON.stringify(mapping);
		try {
			const result =
				samples[name] ??
				(await ipc.getAnkiSampleNote(name, model.fields, { ...draft.anki_connection }));
			if (generation !== ankiGeneration || draft.anki_model_mappings[name] !== mapping) return;
			samples[name] = result;
			delete sampleErrors[name];
			if (suggest && JSON.stringify(mapping) === before) {
				mapping.term_field = result.guessed_term ?? '';
				mapping.reading_field = result.guessed_reading ?? '';
				mapping.sentence_field = result.guessed_sentence;
			}
		} catch (error) {
			if (generation === ankiGeneration) sampleErrors[name] = String(error);
		}
	}

	async function editMapping(name: string) {
		expandedModel = expandedModel === name ? null : name;
		if (!expandedModel) return;
		void loadSample(name);
		await tick();
		document.getElementById(`anki-term-${name}`)?.focus();
	}

	async function addMapping() {
		if (!selectedNewModel || draft.anki_model_mappings[selectedNewModel]) return;
		const name = selectedNewModel;
		if (removed?.name === name) removed = null;
		draft.anki_model_mappings[name] = { term_field: '', reading_field: '', sentence_field: null };
		selectedNewModel = '';
		adding = false;
		expandedModel = name;
		void loadSample(name, true);
		await tick();
		document.getElementById(`anki-term-${name}`)?.focus();
	}

	function removeMapping(name: string) {
		removed = { name, mapping: { ...draft.anki_model_mappings[name] } };
		delete draft.anki_model_mappings[name];
		expandedModel = null;
	}

	function fieldOptions(name: string, value: string | null | undefined) {
		return [
			...new Set([
				...(models.find((m) => m.name === name)?.fields ?? []),
				...(value ? [value] : [])
			])
		];
	}

	function preview(value: string) {
		const plain = value
			.replace(/<\/?(?:b|i|u|em|strong|span|div|font|br)\b[^>]*>/gi, ' ')
			.replace(/&nbsp;/g, ' ')
			.replace(/\s+/g, ' ')
			.trim();
		const chars = [...plain];
		return chars.length > 45 ? chars.slice(0, 42).join('') + '…' : plain;
	}

	async function save() {
		if (saving || !dirty) return;
		saveError = null;
		let invalidField: string | null = null;
		if (!validPort) {
			ankiExpanded = true;
			invalidField = 'anki-port';
			saveError = 'Enter a port from 1 to 65535.';
		} else if (!validUrl(draft.yomitan_url.trim())) {
			yomitanExpanded = true;
			invalidField = 'yomitan-url';
			saveError = 'Enter a valid HTTP or HTTPS URL for Yomitan API.';
		} else if (
			!Number.isInteger(draft.anki_interval) ||
			draft.anki_interval < 1 ||
			draft.anki_interval > 365
		) {
			invalidField = 'anki-interval';
			saveError = 'Enter a review interval from 1 to 365 days.';
		} else {
			for (const [name, mapping] of Object.entries(draft.anki_model_mappings)) {
				if (!mapping.term_field || !mapping.reading_field) {
					expandedModel = name;
					invalidField = `anki-${!mapping.term_field ? 'term' : 'reading'}-${name}`;
					saveError = `Choose the term and reading fields for ${name}.`;
					break;
				}
			}
		}
		if (invalidField) {
			await tick();
			document.getElementById(invalidField)?.focus();
			return;
		}
		saving = true;
		try {
			const snapshot = copyDraft(draft);
			await saveAnkiSettings(
				snapshot.anki_model_mappings,
				snapshot.anki_interval,
				snapshot.yomitan_url.trim(),
				snapshot.anki_connection
			);
			ankiModalOpen.set(false);
		} catch (error) {
			saveError = `Could not save settings: ${String(error)}`;
		} finally {
			saving = false;
		}
	}

	const connectionLabel = (phase: Phase) =>
		({ idle: 'Not tested', loading: 'Testing…', ready: 'Connected', failed: 'Not connected' })[
			phase
		];
</script>

<Modal
	open={$ankiModalOpen}
	title="Anki Settings"
	width="min(760px, 94%)"
	onclose={() => {
		if (!saving) guard.request();
	}}
	oninteract={guard.disarm}
>
	<form
		id="anki-settings-form"
		novalidate
		oninput={() => (saveError = null)}
		onchange={() => (saveError = null)}
		onsubmit={(event) => {
			event.preventDefault();
			void save();
		}}
	>
		<fieldset disabled={saving}>
			<section aria-labelledby="anki-connections-heading">
				<h3 id="anki-connections-heading">Connections</h3>
				<div class="connection">
					<div class="connection-row">
						<div>
							<strong>AnkiConnect / Tsunagi</strong>
							<p class="hint">Read your cards and save mined notes</p>
						</div>
						<span
							class="status"
							class:ok={ankiPhase === 'ready'}
							class:warning={ankiPhase === 'failed'}
							role="status">{connectionLabel(ankiPhase)}</span
						>
						<button
							type="button"
							class="quiet"
							aria-expanded={ankiExpanded}
							aria-controls="anki-connection-panel"
							onclick={() => (ankiExpanded = !ankiExpanded)}
							>{ankiExpanded ? 'Collapse' : 'Configure'}</button
						>
					</div>
					{#if ankiExpanded}
						<div id="anki-connection-panel" class="panel">
							<div class="connection-fields">
								<label for="anki-port">Port</label>
								<div class="input-row">
									<input
										id="anki-port"
										type="number"
										min="1"
										max="65535"
										step="1"
										bind:value={draft.anki_connection.port}
										oninput={editConnection}
										aria-invalid={!validPort}
									/><span class="hint">localhost</span><button
										type="button"
										class="test"
										disabled={!validPort || ankiPhase === 'loading'}
										onclick={testAnki}>Test connection</button
									>
								</div>
							</div>
							{#if !validPort}<p class="error">Enter a whole number from 1 to 65535.</p>{/if}
							<details class="authentication">
								<summary
									>Authentication {draft.anki_connection.api_key
										? '· Key configured'
										: '(optional)'}</summary
								>
								<div class="connection-fields">
									<label for="anki-key">API key</label>
									<div class="input-row">
										<input
											id="anki-key"
											type={showKey ? 'text' : 'password'}
											autocomplete="off"
											spellcheck="false"
											bind:value={draft.anki_connection.api_key}
											oninput={editConnection}
											aria-describedby="anki-key-help"
										/><button
											type="button"
											class="quiet"
											aria-pressed={showKey}
											onclick={() => (showKey = !showKey)}>{showKey ? 'Hide' : 'Show'} key</button
										>
									</div>
								</div>
								<p id="anki-key-help" class="hint">
									Use the key configured in your Anki add-on, or leave blank.
								</p>
							</details>
							{#if ankiError}<p class="error" role="status">{ankiError.message}</p>
								<details>
									<summary>Technical details</summary>
									<p class="detail">{ankiError.detail}</p>
								</details>{:else}<p class="hint">
									Tests use the values above. Save changes to apply them.
								</p>{/if}
						</div>
					{/if}
				</div>
				<div class="connection">
					<div class="connection-row">
						<div>
							<strong>Yomitan API</strong>
							<p class="hint">Build cards with your Yomitan templates</p>
						</div>
						<span
							class="status"
							class:ok={yomitanPhase === 'ready'}
							class:warning={yomitanPhase === 'failed'}
							role="status">{connectionLabel(yomitanPhase)}</span
						><button
							type="button"
							class="quiet"
							aria-expanded={yomitanExpanded}
							aria-controls="yomitan-connection-panel"
							onclick={() => (yomitanExpanded = !yomitanExpanded)}
							>{yomitanExpanded ? 'Collapse' : 'Configure'}</button
						>
					</div>
					{#if yomitanExpanded}<div id="yomitan-connection-panel" class="panel">
							<div class="connection-fields">
								<label for="yomitan-url">URL</label>
								<div class="input-row">
									<input
										id="yomitan-url"
										type="url"
										bind:value={draft.yomitan_url}
										oninput={editYomitan}
									/><button
										type="button"
										disabled={!validUrl(draft.yomitan_url.trim()) || yomitanPhase === 'loading'}
										onclick={testYomitan}>Test connection</button
									>
								</div>
							</div>
							{#if yomitanError}<p class="error" role="status">{yomitanError}</p>{:else}<p
									class="hint"
								>
									{yomitanVersion ? `Yomitan ${yomitanVersion}. ` : ''}Tests use the entered URL.
								</p>{/if}
						</div>{/if}
				</div>
			</section>
			<section aria-labelledby="anki-models-heading">
				<div class="section-heading">
					<h3 id="anki-models-heading">Note types</h3>
					<button
						type="button"
						class="quiet"
						disabled={!availableModels.length}
						onclick={() => (adding = !adding)}>+ Add note type</button
					>
				</div>
				<p class="hint">Choose which fields identify words and sentences already in Anki.</p>
				<div class="catalog-status">
					<span class="hint"
						>{catalogPhase === 'loading'
							? 'Loading note types…'
							: catalogPhase === 'failed'
								? 'Could not load note types. Saved mappings are kept.'
								: catalogPhase === 'ready' && !models.length
									? 'No note types with cards were found.'
									: catalogPhase === 'idle'
										? 'Test the Anki connection to load note types.'
										: `${models.length} note types with cards available`}</span
					><button
						type="button"
						class="reset"
						disabled={!validPort || ankiPhase !== 'ready' || catalogPhase === 'loading'}
						onclick={fetchModels}>Refresh</button
					>
				</div>
				{#if catalogError}<details>
						<summary>Load error details</summary>
						<p class="detail">{catalogError}</p>
					</details>{/if}
				{#if adding}<div class="add-row">
						<label for="anki-new-model">Note type</label><select
							id="anki-new-model"
							bind:value={selectedNewModel}
							onchange={addMapping}
							><option value="" disabled>Choose a note type…</option
							>{#each availableModels as model (model.name)}<option value={model.name}
									>{model.name}</option
								>{/each}</select
						><button type="button" class="quiet" onclick={() => (adding = false)}>Dismiss</button>
					</div>{/if}
				{#each Object.entries(draft.anki_model_mappings) as [name, mapping] (name)}
					<div class="mapping">
						<div class="mapping-row">
							<div>
								<strong>{name}</strong>
								<p class="hint mapping-summary">
									{mapping.term_field || 'Choose term field'} · {mapping.reading_field ||
										'Choose reading field'}{mapping.sentence_field
										? ` · ${mapping.sentence_field}`
										: ''}
								</p>
							</div>
							<button
								type="button"
								class="quiet"
								aria-label={`Edit ${name} mapping`}
								aria-expanded={expandedModel === name}
								aria-controls={`anki-mapping-${name}`}
								onclick={() => editMapping(name)}
								>{expandedModel === name ? 'Collapse' : 'Edit'}</button
							>
						</div>
						{#if expandedModel === name}<div id={`anki-mapping-${name}`} class="panel">
								{#if !models.some((m) => m.name === name)}<p class="hint">
										Field choices are unavailable. Reconnect and refresh to load this note type.
									</p>{/if}
								<div class="mapping-fields">
									{#each [['term_field', 'Term', 'term'], ['reading_field', 'Reading', 'reading'], ['sentence_field', 'Sentence (optional)', 'sentence']] as [key, label, id]}
										{@const field = key as keyof ipc.FieldMapping}
										{@const value = mapping[field]}
										{@const example = value ? samples[name]?.sample_note?.[value] : undefined}
										<div class="field">
											<label for={`anki-${id}-${name}`}>{label}</label><select
												id={`anki-${id}-${name}`}
												bind:value={mapping[field]}
												><option
													value={field === 'sentence_field' ? null : ''}
													disabled={field !== 'sentence_field'}
													>{field === 'sentence_field' ? 'None' : 'Choose a field…'}</option
												>{#each fieldOptions(name, value) as option}<option value={option}
														>{option}</option
													>{/each}</select
											>{#if example !== undefined}<span class="example" title={example}
													>{preview(example)}</span
												>{/if}
										</div>
									{/each}
								</div>
								{#if sampleErrors[name]}<p class="hint">
										Could not load the sample note. Your field selections are kept.
									</p>
									<button type="button" class="reset" onclick={() => loadSample(name)}
										>Retry sample</button
									>{:else if samples[name]?.sample_note === null}<p class="hint">
										No sample note is available.
									</p>{/if}
								<div class="mapping-actions">
									<button type="button" class="reset danger" onclick={() => removeMapping(name)}
										>Remove mapping</button
									><span class="hint">Anki notes are kept.</span>
								</div>
							</div>{/if}
					</div>
				{:else}<p class="hint">
						No note types configured. Add one to recognize words from your Anki cards.
					</p>{/each}
				{#if removed}<div class="removed">
						<span class="hint">Removed {removed.name} mapping.</span><button
							type="button"
							class="reset"
							onclick={() => {
								if (removed) {
									draft.anki_model_mappings[removed.name] = removed.mapping;
									removed = null;
								}
							}}>Undo</button
						>
					</div>{/if}
			</section>
			<section class="estimate" aria-labelledby="anki-interval-heading">
				<div>
					<h3 id="anki-interval-heading">Comprehension estimate</h3>
					<p class="hint">
						Cards count fully at this review interval. Shorter intervals contribute partial credit.
					</p>
				</div>
				<div class="input-row">
					<input
						id="anki-interval"
						type="number"
						min="1"
						max="365"
						step="1"
						bind:value={draft.anki_interval}
						aria-label="Review interval for full comprehension"
					/><span class="hint">days</span>
				</div>
			</section>
		</fieldset>
	</form>
	{#snippet footer()}
		<hr />
		{#if saveError}<p class="save-error error" role="alert">{saveError}</p>{/if}
		<div class="dirty" role="status">
			{#if guard.armed}⚠ Unsaved changes — dismiss again to discard{:else if dirty}⚠ Settings have
				been modified{/if}
		</div>
		<footer>
			<button
				class="primary"
				type="submit"
				form="anki-settings-form"
				disabled={!dirty || !validPort || saving}>Save Settings</button
			>
			<button disabled={!dirty || saving} onclick={revert}>Cancel</button>
			<button class="right" disabled={saving} onclick={restoreDefault}>Restore Default</button>
		</footer>
	{/snippet}
</Modal>

<style>
	form {
		padding: 0 1rem;
	}
	fieldset {
		border: 0;
		padding: 0;
		margin: 0;
		min-width: 0;
		display: grid;
		gap: 1.25rem;
	}
	section {
		min-width: 0;
	}
	h3 {
		margin: 0;
		font-size: 0.95rem;
	}
	p {
		margin: 0.3rem 0 0;
	}
	.hint,
	.example {
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.45;
	}
	.section-heading,
	.mapping-row,
	.mapping-actions,
	.catalog-status,
	.removed,
	.input-row,
	.add-row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}
	.section-heading,
	.mapping-row,
	.catalog-status {
		justify-content: space-between;
	}
	.connection,
	.mapping {
		border: 1px solid var(--border);
		border-radius: var(--radius);
		margin-top: 0.5rem;
	}
	.connection-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 7.5rem 6.5rem;
		align-items: center;
		gap: 0.5rem;
		padding: 0.65rem 0.75rem;
	}
	strong {
		font-size: 0.88rem;
	}
	.mapping-row {
		padding: 0.55rem 0.75rem;
	}
	.mapping-row > div {
		min-width: 0;
	}
	.mapping-summary {
		overflow-wrap: anywhere;
	}
	.mapping-row strong {
		color: var(--info);
	}
	.panel {
		margin: 0 0.75rem;
		padding: 0.7rem 0;
		border-top: 1px solid var(--border);
	}
	.connection-fields {
		display: grid;
		grid-template-columns: 6rem minmax(0, 1fr);
		align-items: center;
		gap: 0.65rem;
	}
	.input-row {
		min-width: 0;
	}
	.input-row input:not([type='number']) {
		flex: 1;
		width: 0;
	}
	.test {
		margin-left: auto;
	}
	input,
	select {
		min-width: 0;
		max-width: 100%;
		padding: 0.4rem 0.5rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font: inherit;
	}
	input[type='number'] {
		width: 5.5rem;
	}
	label {
		font-size: 0.85rem;
	}
	.authentication {
		margin: 0.7rem 0;
	}
	.authentication .connection-fields {
		margin-top: 0.65rem;
	}
	summary {
		cursor: pointer;
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	details {
		margin-top: 0.5rem;
	}
	.detail {
		overflow-wrap: anywhere;
		white-space: pre-wrap;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.status {
		font-size: 0.78rem;
		color: var(--text-muted);
	}
	.status::before {
		content: '●';
		font-size: 0.6rem;
		margin-right: 0.4rem;
	}
	.ok {
		color: var(--success);
	}
	.warning {
		color: var(--warning);
	}
	.error {
		color: var(--danger);
		font-size: 0.85rem;
		overflow-wrap: anywhere;
	}
	.quiet,
	.reset {
		background: transparent;
		border-color: transparent;
	}
	.reset {
		color: var(--text-muted);
		font-size: 0.8rem;
		padding: 0.25rem 0;
	}
	.danger {
		color: var(--danger);
	}
	.catalog-status {
		margin: 0.35rem 0;
	}
	.mapping-fields {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.75rem;
	}
	.field {
		display: grid;
		align-content: start;
		gap: 0.35rem;
		min-width: 0;
	}
	.example {
		overflow-wrap: anywhere;
	}
	.mapping-actions {
		margin-top: 0.7rem;
	}
	.add-row,
	.removed {
		margin-top: 0.6rem;
	}
	.estimate {
		display: flex;
		align-items: center;
		gap: 1.2rem;
	}
	.estimate > div:first-child {
		flex: 1;
	}
	hr {
		border: none;
		border-top: 1px solid var(--border);
		margin: 0 1rem;
	}
	.save-error {
		margin: 0.5rem 1rem;
	}
	.dirty {
		min-height: 1.2rem;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--warning);
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
	@media (max-width: 640px) {
		.connection-row {
			grid-template-columns: minmax(0, 1fr) 6.5rem;
		}
		.connection-row .status {
			grid-column: 1;
			grid-row: 2;
		}
		.connection-row button {
			grid-column: 2;
			grid-row: 1 / 3;
		}
		.connection-fields,
		.mapping-fields {
			grid-template-columns: 1fr;
		}
		.estimate {
			align-items: start;
			flex-direction: column;
			gap: 0.5rem;
		}
		.input-row {
			flex-wrap: wrap;
		}
		.input-row input:not([type='number']) {
			min-width: 8rem;
		}
	}
</style>
