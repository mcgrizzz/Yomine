<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import * as ipc from '$lib/ipc';
	import {
		ankiFocus,
		ankiModalOpen,
		defaultSettings,
		settings,
		saveAnkiSettings
	} from '$lib/stores';

	type Draft = Pick<
		ipc.SettingsData,
		'anki_connection' | 'anki_model_mappings' | 'anki_interval' | 'yomitan_url'
	>;
	type Phase = 'idle' | 'loading' | 'ready' | 'failed';
	function copyDraft(s: Draft): Draft {
		return {
			anki_connection: { ...s.anki_connection },
			anki_model_mappings: Object.fromEntries(
				Object.entries(s.anki_model_mappings).map(([name, fields]) => [
					name,
					{
						...fields,
						sentence_field: fields.sentence_field ?? null,
						sentence_audio_field: fields.sentence_audio_field ?? null,
						picture_field: fields.picture_field ?? null
					}
				])
			),
			anki_interval: s.anki_interval,
			yomitan_url: s.yomitan_url
		};
	}
	// Replaced from the saved settings each time the dialog opens.
	const form = settingsDraft<Draft>({
		open: ankiModalOpen,
		initial: {
			anki_connection: { host: '', port: 0, api_key: '' },
			anki_model_mappings: {},
			anki_interval: 0,
			yomitan_url: ''
		}
	});
	const draft = $derived(form.value);
	let ankiExpanded = $state(false);
	let yomitanExpanded = $state(false);
	let showKey = $state(false);
	let expandedModel = $state<string | null>(null);
	let adding = $state(false);
	let pickerStyle = $state('');
	let modelSearch = $state('');
	let modelIndex = $state(0);
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

	const validPort = $derived(
		Number.isInteger(draft.anki_connection.port) &&
			draft.anki_connection.port >= 1 &&
			draft.anki_connection.port <= 65535
	);
	const validHost = $derived(isValidHost(draft.anki_connection.host));
	function isValidHost(value: string) {
		const host = value.trim();
		if (!host || /[\s/\\@?#]/.test(host) || (host.startsWith('[') && !host.endsWith(']')))
			return false;
		try {
			const authority = host.includes(':') && !host.startsWith('[') ? `[${host}]` : host;
			const url = new URL(`http://${authority}/`);
			return !url.port && url.host === url.hostname;
		} catch {
			return false;
		}
	}
	const availableModels = $derived(
		models.filter((model) => !draft.anki_model_mappings[model.name])
	);
	const filteredModels = $derived(
		availableModels.filter((model) =>
			model.name.toLocaleLowerCase().includes(modelSearch.trim().toLocaleLowerCase())
		)
	);
	const activeModel = $derived(filteredModels[modelIndex]);

	$effect(() => {
		if (!$ankiModalOpen) return;
		untrack(hydrate);
		return () => {
			ankiGeneration++;
			yomitanGeneration++;
			stopAudio();
		};
	});

	function hydrate() {
		const saved = $settings ?? $defaultSettings;
		if (saved) form.reset(copyDraft(saved));
		revert();
		if ($ankiFocus && draft.anki_model_mappings[$ankiFocus]) {
			expandedModel = $ankiFocus;
			// Without a cached catalog this returns early; fetchModels loads it instead.
			void loadSample($ankiFocus);
		}
		ankiFocus.set(null);
	}

	function revert() {
		form.revert();
		ankiGeneration++;
		yomitanGeneration++;
		if (catalogConnection !== JSON.stringify(draft.anki_connection)) clearCatalog();
		else if (catalogPhase === 'loading') catalogPhase = 'idle';
		expandedModel = null;
		adding = false;
		modelSearch = '';
		modelIndex = 0;
		removed = null;
		showKey = false;
		saveError = null;
		form.disarm();
		void testAnki();
		void testYomitan();
	}

	function clearCatalog() {
		adding = false;
		models = [];
		samples = {};
		sampleErrors = {};
		catalogPhase = 'idle';
		catalogError = null;
		catalogConnection = '';
	}

	function restoreDefault() {
		if (!$defaultSettings) return;
		form.value = copyDraft($defaultSettings);
		expandedModel = null;
		adding = false;
		modelSearch = '';
		modelIndex = 0;
		removed = null;
		showKey = false;
		saveError = null;
		editConnection();
		editYomitan();
		form.disarm();
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
		if (!validHost || !validPort) return;
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
			modelIndex = 0;
			catalogConnection = JSON.stringify(connection);
			catalogPhase = 'ready';
			if (expandedModel) void loadSample(expandedModel);
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
				(await ipc.getAnkiSampleNote(
					name,
					model.fields,
					{ ...draft.anki_connection },
					draft.yomitan_url.trim()
				));
			if (generation !== ankiGeneration || draft.anki_model_mappings[name] !== mapping) return;
			samples[name] = result;
			delete sampleErrors[name];
			if (suggest && JSON.stringify(mapping) === before) {
				mapping.term_field = result.guessed_term ?? '';
				mapping.reading_field = result.guessed_reading ?? '';
				mapping.sentence_field = result.guessed_sentence;
			}
			// Mappings saved before local mining have neither media field.
			if (!mapping.sentence_audio_field && !mapping.picture_field) {
				mapping.sentence_audio_field = result.guessed_sentence_audio;
				mapping.picture_field = result.guessed_picture;
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

	async function toggleAdding() {
		adding = !adding;
		modelSearch = '';
		modelIndex = 0;
		if (adding) {
			const anchor = document.getElementById('anki-add-model')!.getBoundingClientRect();
			const zoom = Number(getComputedStyle(document.documentElement).zoom) || 1;
			const width = Math.min(420, window.innerWidth / zoom - 16);
			const below = (window.innerHeight - anchor.bottom) / zoom - 14;
			const above = below < 280 && anchor.top / zoom > below;
			pickerStyle = `width: ${width}px; left: ${Math.max(8, anchor.right / zoom - width)}px; ` +
				(above
					? `bottom: ${(window.innerHeight - anchor.top) / zoom + 6}px; max-height: ${anchor.top / zoom - 14}px;`
					: `top: ${anchor.bottom / zoom + 6}px; max-height: ${below}px;`);
		}
		await tick();
		document.getElementById(adding ? 'anki-model-search' : 'anki-add-model')?.focus();
	}

	$effect(() => {
		if (!adding) return;
		function dismissPicker(event: Event) {
			if (event.target instanceof Element && event.target.closest('#anki-model-picker, #anki-add-model')) return;
			adding = false;
		}
		const events = ['pointerdown', 'focusin', 'scroll', 'resize'];
		for (const event of events) window.addEventListener(event, dismissPicker, true);
		return () => {
			for (const event of events) window.removeEventListener(event, dismissPicker, true);
		};
	});

	async function searchKeydown(event: KeyboardEvent) {
		if (event.isComposing) return;
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			void toggleAdding();
		} else if (event.key === 'Enter') {
			event.preventDefault();
			if (activeModel) void addMapping(activeModel.name);
		} else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			modelIndex = Math.max(
				0,
				Math.min(filteredModels.length - 1, modelIndex + (event.key === 'ArrowDown' ? 1 : -1))
			);
			await tick();
			document.getElementById(`anki-model-option-${modelIndex}`)?.scrollIntoView({ block: 'nearest' });
		}
	}

	async function addMapping(name: string) {
		if (!availableModels.some((model) => model.name === name)) return;
		if (removed?.name === name) removed = null;
		draft.anki_model_mappings[name] = emptyMapping();
		modelSearch = '';
		modelIndex = 0;
		adding = false;
		expandedModel = name;
		void loadSample(name, true);
		await tick();
		document.getElementById(`anki-term-${name}`)?.focus();
	}

	function emptyMapping(): ipc.FieldMapping {
		return {
			term_field: '',
			reading_field: '',
			sentence_field: null,
			sentence_audio_field: null,
			picture_field: null
		};
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

	// Anki stores media in fields as [sound:clip.mp3] and <img src="shot.jpg">.
	function soundFile(value: string) {
		return value.match(/\[sound:([^\]]+)\]/)?.[1] ?? null;
	}
	function imageFile(value: string) {
		const src = value.match(/<img[^>]*?\ssrc=["']?([^"'\s>]+)/i)?.[1];
		if (!src) return null;
		try {
			return decodeURIComponent(src.replace(/&amp;/g, '&'));
		} catch {
			return src;
		}
	}

	const mediaUris = new Map<string, Promise<string | null>>();
	function mediaUri(file: string) {
		let uri = mediaUris.get(file);
		if (!uri) {
			uri = ipc.getMediaPreview(file).catch(() => null);
			mediaUris.set(file, uri);
		}
		return uri;
	}

	let audio: HTMLAudioElement | null = null;
	let playing = $state<string | null>(null);
	async function togglePlay(file: string) {
		const wasPlaying = playing === file;
		stopAudio();
		if (wasPlaying) return;
		playing = file;
		const uri = await mediaUri(file);
		if (playing !== file) return;
		if (!uri) {
			playing = null;
			return;
		}
		audio = new Audio(uri);
		audio.onended = () => {
			if (playing === file) playing = null;
		};
		audio.play().catch(() => (playing = null));
	}
	function stopAudio() {
		audio?.pause();
		audio = null;
		playing = null;
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
		if (saving || !form.dirty) return;
		saveError = null;
		let invalidField: string | null = null;
		if (!validHost) {
			ankiExpanded = true;
			invalidField = 'anki-host';
			saveError = 'Enter a hostname or IP address without a URL or port.';
		} else if (!validPort) {
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
			snapshot.anki_connection.host = snapshot.anki_connection.host.trim();
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

{#snippet fieldSelect(
	name: string,
	mapping: ipc.FieldMapping,
	field: keyof ipc.FieldMapping,
	id: string
)}
	{@const optional = field !== 'term_field' && field !== 'reading_field'}
	<select id={`anki-${id}-${name}`} bind:value={mapping[field]}
		><option value={optional ? null : ''} disabled={!optional}
			>{optional ? 'None' : 'Choose a field…'}</option
		>{#each fieldOptions(name, mapping[field]) as option}<option value={option}>{option}</option
			>{/each}</select
	>
{/snippet}

<Modal
	open={$ankiModalOpen}
	title="Anki Settings"
	width="min(760px, 94%)"
	onclose={() => {
		if (adding) void toggleAdding();
		else if (!saving) form.request();
	}}
	oninteract={form.disarm}
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
								<label for="anki-host">Host / IP</label>
								<div class="input-row">
									<input
										id="anki-host"
										type="text"
										spellcheck="false"
										autocapitalize="none"
										placeholder="localhost"
										bind:value={draft.anki_connection.host}
										oninput={editConnection}
										aria-invalid={!validHost}
										aria-describedby="anki-host-help"
									/>
								</div>
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
									/><button
										type="button"
										class="test"
										disabled={!validHost || !validPort || ankiPhase === 'loading'}
										onclick={testAnki}>Test connection</button
									>
								</div>
							</div>
							<p id="anki-host-help" class="hint">
								Use localhost for this computer, or the address of the computer running Anki.
							</p>
							{#if !validHost}<p class="error">
									Enter a hostname or IP address without a URL or port.
								</p>{/if}
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
						id="anki-add-model"
						type="button"
						class="quiet"
						aria-expanded={adding}
						aria-controls="anki-model-picker"
						disabled={!availableModels.length}
						onclick={toggleAdding}>+ Add note type</button
					>
				</div>
				<p class="hint">
					Choose which fields identify words and sentences already in Anki, and where local mining
					puts sentence audio and screenshots.
				</p>
				<div class="catalog-status">
					<span class="hint"
						>{catalogPhase === 'loading'
							? 'Loading note types…'
							: catalogPhase === 'failed'
								? 'Could not load note types. Saved mappings are kept.'
								: catalogPhase === 'ready' && !models.length
									? 'No note types were found.'
									: catalogPhase === 'idle'
										? 'Test the Anki connection to load note types.'
										: `${models.length} note types available`}</span
					><button
						type="button"
						class="reset"
						disabled={!validHost ||
							!validPort ||
							ankiPhase !== 'ready' ||
							catalogPhase === 'loading'}
						onclick={fetchModels}>Refresh</button
					>
				</div>
				{#if catalogError}<details>
						<summary>Load error details</summary>
						<p class="detail">{catalogError}</p>
					</details>{/if}
				{#if adding}<div id="anki-model-picker" class="model-picker" style={pickerStyle}>
						<label for="anki-model-search">Search note types</label>
						<div class="add-row">
							<input
								id="anki-model-search"
								role="combobox"
								aria-autocomplete="list"
								aria-expanded="true"
								aria-controls="anki-model-options"
								aria-activedescendant={activeModel ? `anki-model-option-${modelIndex}` : undefined}
								bind:value={modelSearch}
								oninput={() => {
									modelIndex = 0;
									document.getElementById('anki-model-options')?.scrollTo(0, 0);
								}}
								onkeydown={searchKeydown}
								autocomplete="off"
								placeholder="Type a name…"
							/>
							<button type="button" class="quiet" onclick={toggleAdding}>Dismiss</button>
						</div>
						<div id="anki-model-options" class="model-options" role="listbox" aria-label="Note types">
							{#each filteredModels as model, index (model.name)}
								<button
									id={`anki-model-option-${index}`}
									type="button"
									role="option"
									aria-selected={index === modelIndex}
									tabindex="-1"
									onpointermove={() => (modelIndex = index)}
									onclick={() => addMapping(model.name)}>{model.name}</button
								>
							{/each}
						</div>
						{#if !filteredModels.length}<p class="hint" role="status">No matching note types.</p>{/if}
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
								{#if samples[name]?.detected}<p class="hint recognised">
										{samples[name].fields_differ
											? `Recognised as ${samples[name].detected}, but some of its usual fields are missing. Check the ones below.`
											: `Recognised as ${samples[name].detected}.`}
									</p>{/if}
								<h4>Cards already in Anki</h4>
								<div class="mapping-fields">
									{#each [['term_field', 'Term', 'term'], ['reading_field', 'Reading', 'reading'], ['sentence_field', 'Sentence (optional)', 'sentence']] as [key, label, id]}
										{@const field = key as keyof ipc.FieldMapping}
										{@const value = mapping[field]}
										{@const example = value ? samples[name]?.sample_note?.[value] : undefined}
										<div class="field">
											<label for={`anki-${id}-${name}`}>{label}</label>
											{@render fieldSelect(name, mapping, field, id)}
											{#if example !== undefined}<span class="example" title={example}
													>{preview(example)}</span
												>{/if}
										</div>
									{/each}
								</div>
								<h4 class="media-heading">Local mining</h4>
								<div class="media-rows">
									{#each [['sentence_audio_field', 'Sentence audio', 'sentence-audio'], ['picture_field', 'Picture', 'picture']] as [key, label, id]}
										{@const field = key as keyof ipc.FieldMapping}
										{@const value = mapping[field]}
										{@const example = value ? samples[name]?.sample_note?.[value] : undefined}
										{@const sound =
											field === 'sentence_audio_field' && example ? soundFile(example) : null}
										{@const image = field === 'picture_field' && example ? imageFile(example) : null}
										<label for={`anki-${id}-${name}`}>{label}</label>
										{@render fieldSelect(name, mapping, field, id)}
										<div class="sample">
											{#if sound}<button
													type="button"
													class="play"
													title={sound}
													aria-pressed={playing === sound}
													onclick={() => togglePlay(sound)}
													><span aria-hidden="true">{playing === sound ? '■' : '▶'}</span
													>{playing === sound ? 'Stop' : 'Play sample'}</button
												>{:else if image}{#await mediaUri(image)}<span class="frame"></span
													>{:then uri}{#if uri}<img
															class="frame"
															src={uri}
															alt="Sample note's screenshot"
															title={image}
														/>{:else}<span class="example">{image}</span>{/if}{/await}{:else if example}<span
													class="example"
													title={example}>{preview(example)}</span
												>{:else if example === ''}<span class="hint">Empty in the sample note</span
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
										No sample note available yet.
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
		{#if saveError}<p class="save-error error" role="alert">{saveError}</p>{/if}
		<SettingsFooter
			{form}
			invalid={!validPort}
			busy={saving}
			submits="anki-settings-form"
			oncancel={revert}
			onrestore={restoreDefault}
		/>
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
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.play {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.3rem 0.75rem 0.3rem 0.6rem;
		border-radius: 999px;
		font-size: 0.8rem;
	}
	.play span {
		font-size: 0.65rem;
	}
	.mapping-actions {
		margin-top: 0.7rem;
	}
	.add-row,
	.removed {
		margin-top: 0.6rem;
	}
	.add-row input {
		flex: 1;
		width: 0;
	}
	.model-picker {
		position: fixed;
		z-index: var(--z-popover);
		display: flex;
		flex-direction: column;
		padding: 0.75rem;
		background: var(--bg-panel);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
	}
	.model-options {
		display: flex;
		flex-direction: column;
		min-height: 0;
		max-height: 14rem;
		overflow-y: auto;
		overscroll-behavior: contain;
		margin-top: 0.35rem;
	}
	.model-options button {
		flex-shrink: 0;
		text-align: left;
		overflow-wrap: anywhere;
		background: transparent;
		border-color: transparent;
		font-weight: normal;
		cursor: pointer;
	}
	.model-options button:hover,
	.model-options button[aria-selected='true'] {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.recognised {
		margin: 0 0 0.8rem;
	}
	h4 {
		margin: 0 0 0.5rem;
		font-size: 0.8rem;
		font-weight: 600;
		color: var(--text-muted);
	}
	.media-heading {
		margin-top: 1rem;
		padding-top: 0.8rem;
		border-top: 1px dashed var(--border);
	}
	.media-rows {
		display: grid;
		grid-template-columns: 7rem minmax(0, 15rem) minmax(0, 1fr);
		align-items: center;
		gap: 0.65rem 0.9rem;
	}
	.sample {
		display: flex;
		align-items: center;
		min-width: 0;
		min-height: 2.2rem;
	}
	.frame {
		display: block;
		width: 8.5rem;
		aspect-ratio: 16 / 9;
		object-fit: cover;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.estimate {
		display: flex;
		align-items: center;
		gap: 1.2rem;
	}
	.estimate > div:first-child {
		flex: 1;
	}
	.save-error {
		margin: 0.5rem 1rem;
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
		.mapping-fields,
		.media-rows {
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
