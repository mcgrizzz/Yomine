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
	type AnkiProbe = {
		phase: Phase;
		backend: ipc.AnkiBackend | null;
		missing: ipc.MissingPermissions | null;
		error: ipc.ConnectionError | null;
	};
	type YomitanProbe = { phase: Phase; version: string | null; error: string | null };
	const UNTESTED_ANKI: AnkiProbe = { phase: 'idle', backend: null, missing: null, error: null };
	const UNTESTED_YOMITAN: YomitanProbe = { phase: 'idle', version: null, error: null };
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
						picture_field: fields.picture_field ?? null,
						source_field: fields.source_field ?? null
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
	let editingModel = $state<string | null>(null);
	/** Note types whose fields were filled from Yomine's guess, by adding or resetting them. */
	let autoFilled = $state<Set<string>>(new Set());
	let showPreview = $state(false);
	let adding = $state(false);
	let pickerStyle = $state('');
	let modelSearch = $state('');
	let modelIndex = $state(0);
	let removed = $state<{ name: string; mapping: ipc.FieldMapping } | null>(null);
	let saving = $state(false);
	let saveError = $state<string | null>(null);
	// The headers show the saved connections, and Test the entered values. Opening the dialog
	// tests the saved values into both; the panel shows only a probe Test ran since.
	let ankiActive = $state.raw(UNTESTED_ANKI);
	let ankiTest = $state.raw(UNTESTED_ANKI);
	let yomitanActive = $state.raw(UNTESTED_YOMITAN);
	let yomitanTest = $state.raw(UNTESTED_YOMITAN);
	let catalogPhase = $state<Phase>('idle');
	let catalogError = $state<string | null>(null);
	let models = $state<ipc.AnkiModelInfo[]>([]);
	let samples = $state<Record<string, ipc.SampleNote>>({});
	let sampleErrors = $state<Record<string, string>>({});
	let catalogConnection = '';
	let ankiGeneration = 0;
	let yomitanGeneration = 0;
	let activeGeneration = 0;

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

	const noteTypeCounts = $derived(
		`${Object.keys(draft.anki_model_mappings).length} configured · ` +
			(catalogPhase === 'loading'
				? 'loading note types…'
				: catalogPhase === 'failed'
					? 'could not load note types'
					: catalogPhase === 'idle'
						? 'test the Anki connection to list the others'
						: `${availableModels.length} available`)
	);

	$effect(() => {
		if (!$ankiModalOpen) return;
		untrack(hydrate);
		return () => {
			ankiGeneration++;
			yomitanGeneration++;
			activeGeneration++;
			stopAudio();
		};
	});

	function hydrate() {
		const saved = $settings ?? $defaultSettings;
		if (saved) form.reset(copyDraft(saved));
		editingModel = null;
		revert();
		if ($ankiFocus && draft.anki_model_mappings[$ankiFocus]) {
			editingModel = $ankiFocus;
			// Without a cached catalog this returns early; fetchModels loads it instead.
			void loadSample($ankiFocus);
		}
		ankiFocus.set(null);
	}

	function revert() {
		form.revert();
		ankiGeneration++;
		yomitanGeneration++;
		activeGeneration++;
		if (catalogConnection !== JSON.stringify(draft.anki_connection)) clearCatalog();
		else if (catalogPhase === 'loading') catalogPhase = 'idle';
		if (editingModel && !draft.anki_model_mappings[editingModel]) editingModel = null;
		autoFilled = new Set();
		adding = false;
		modelSearch = '';
		modelIndex = 0;
		removed = null;
		showKey = false;
		saveError = null;
		form.disarm();
		void testAnki(true);
		void testYomitan(true);
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
		editingModel = null;
		autoFilled = new Set();
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
		ankiTest = UNTESTED_ANKI;
		clearCatalog();
	}

	function editYomitan() {
		yomitanGeneration++;
		yomitanTest = UNTESTED_YOMITAN;
	}

	/** Tests the entered values; `active` when they are the saved ones. */
	async function testAnki(active = false) {
		if (!validHost || !validPort) return;
		const entered = ankiGeneration;
		const saved = activeGeneration;
		ankiTest = { ...UNTESTED_ANKI, phase: 'loading' };
		if (active) ankiActive = ankiTest;
		let probe: AnkiProbe;
		try {
			const report = await ipc.testAnkiConnection({ ...draft.anki_connection });
			probe = { phase: 'ready', backend: report.backend, missing: report.missing, error: null };
		} catch (error) {
			probe = {
				...UNTESTED_ANKI,
				phase: 'failed',
				error:
					typeof error === 'object' && error !== null && 'message' in error && 'detail' in error
						? (error as ipc.ConnectionError)
						: {
								message: 'Connection test failed. Check the Anki add-on settings.',
								detail: String(error)
							}
			};
		}
		if (active && saved === activeGeneration) ankiActive = probe;
		if (entered !== ankiGeneration) return;
		ankiTest = probe;
		if (probe.phase === 'ready' && catalogPhase !== 'ready') await fetchModels();
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
			if (editingModel) void loadSample(editingModel);
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

	async function testYomitan(active = false) {
		const url = draft.yomitan_url.trim();
		if (!validUrl(url)) return;
		const entered = yomitanGeneration;
		const saved = activeGeneration;
		yomitanTest = { ...UNTESTED_YOMITAN, phase: 'loading' };
		if (active) yomitanActive = yomitanTest;
		let probe: YomitanProbe;
		try {
			const result = await ipc.getYomitanStatus(url);
			probe = result.reachable
				? { phase: 'ready', version: result.version, error: null }
				: {
						phase: 'failed',
						version: null,
						error:
							'Cannot reach Yomitan API. Check that the companion is running and the URL is correct.'
					};
		} catch (error) {
			probe = { phase: 'failed', version: null, error: String(error) };
		}
		if (active && saved === activeGeneration) yomitanActive = probe;
		if (entered === yomitanGeneration) yomitanTest = probe;
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
			if (suggest && JSON.stringify(mapping) === before) applyGuess(name, mapping, result);
			// Mappings saved before local mining have neither media field.
			if (!mapping.sentence_audio_field && !mapping.picture_field) {
				mapping.sentence_audio_field = result.guessed_sentence_audio;
				mapping.picture_field = result.guessed_picture;
			}
			// Mappings saved before the source field existed.
			mapping.source_field ??= result.guessed_source ?? '';
		} catch (error) {
			if (generation === ankiGeneration) sampleErrors[name] = String(error);
		}
	}

	function applyGuess(name: string, mapping: ipc.FieldMapping, guess: ipc.SampleNote) {
		mapping.term_field = guess.guessed_term ?? '';
		mapping.reading_field = guess.guessed_reading ?? '';
		mapping.sentence_field = guess.guessed_sentence;
		mapping.sentence_audio_field = guess.guessed_sentence_audio;
		mapping.picture_field = guess.guessed_picture;
		mapping.source_field = guess.guessed_source ?? '';
		if (Object.values(mapping).some(Boolean)) autoFilled = new Set(autoFilled).add(name);
	}

	async function resetMapping(name: string) {
		await loadSample(name);
		const mapping = draft.anki_model_mappings[name];
		if (samples[name] && mapping) applyGuess(name, mapping, samples[name]);
	}

	async function editMapping(name: string) {
		editingModel = name;
		void loadSample(name);
		await showEditor(name);
	}

	async function showEditor(name: string) {
		await tick();
		document.getElementById('anki-settings-form')?.closest('.content')?.scrollTo(0, 0);
		document.getElementById(`anki-term-${name}`)?.focus();
	}

	async function closeEditor() {
		const name = editingModel;
		editingModel = null;
		stopAudio();
		await tick();
		document.getElementById(`anki-edit-${name}`)?.focus();
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
			pickerStyle =
				`width: ${width}px; left: ${Math.max(8, Math.min(anchor.left / zoom, window.innerWidth / zoom - width - 8))}px; ` +
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
			if (
				event.target instanceof Element &&
				event.target.closest('#anki-model-picker, #anki-add-model')
			)
				return;
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
			document
				.getElementById(`anki-model-option-${modelIndex}`)
				?.scrollIntoView({ block: 'nearest' });
		}
	}

	async function addMapping(name: string) {
		if (!availableModels.some((model) => model.name === name)) return;
		if (removed?.name === name) removed = null;
		draft.anki_model_mappings[name] = emptyMapping();
		modelSearch = '';
		modelIndex = 0;
		adding = false;
		editingModel = name;
		void loadSample(name, true);
		await showEditor(name);
	}

	function emptyMapping(): ipc.FieldMapping {
		return {
			term_field: '',
			reading_field: '',
			sentence_field: null,
			sentence_audio_field: null,
			picture_field: null,
			source_field: null
		};
	}

	function removeMapping(name: string) {
		removed = { name, mapping: { ...draft.anki_model_mappings[name] } };
		delete draft.anki_model_mappings[name];
		editingModel = null;
		stopAudio();
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
		let invalidModel: string | null = null;
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
					invalidModel = name;
					invalidField = `anki-${!mapping.term_field ? 'term' : 'reading'}-${name}`;
					saveError = `Choose the term and reading fields for ${name}.`;
					break;
				}
			}
		}
		if (invalidField) {
			editingModel = invalidModel;
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

	const connectionLabel = (phase: Phase, detail: string | null) =>
		({ idle: 'Not tested', loading: 'Testing…', ready: 'Connected', failed: 'Not connected' })[
			phase
		] + (detail ? ` · ${detail}` : '');
	const backendLabel = ({ backend }: AnkiProbe) =>
		backend && (backend.kind === 'tsunagi' ? `Tsunagi ${backend.version}` : 'AnkiConnect');
</script>

{#snippet fieldSelect(
	name: string,
	mapping: ipc.FieldMapping,
	field: keyof ipc.FieldMapping,
	id: string
)}
	{@const optional = field !== 'term_field' && field !== 'reading_field'}
	<select id={`anki-${id}-${name}`} bind:value={mapping[field]}
		><option value={optional && field !== 'source_field' ? null : ''} disabled={!optional}
			>{optional ? 'None' : 'Choose a field…'}</option
		>{#each fieldOptions(name, mapping[field]) as option}<option value={option}>{option}</option
			>{/each}</select
	>
{/snippet}

{#snippet editor(name: string, mapping: ipc.FieldMapping)}
	{@const sample = samples[name]}
	<div class="editor">
		{#if !models.some((m) => m.name === name)}<p class="hint">
				Field choices are unavailable. Reconnect and refresh to load this note type.
			</p>{/if}
		{#if autoFilled.has(name)}<p class="hint">
				{sample?.fields_differ
					? `Field mappings detected automatically, but some of ${sample.detected}'s usual fields are missing. Check them below.`
					: 'Field mappings detected automatically.'}
			</p>{/if}
		<section aria-labelledby="anki-read-heading">
			<h4 id="anki-read-heading">Read from Anki</h4>
			<p class="hint">Yomine finds the words and sentences you already have in these fields.</p>
			<div class="mapping-fields">
				{#each [['term_field', 'Term', 'term'], ['reading_field', 'Reading', 'reading'], ['sentence_field', 'Sentence (optional)', 'sentence']] as [key, label, id]}
					{@const field = key as keyof ipc.FieldMapping}
					{@const value = mapping[field]}
					{@const example = value ? sample?.sample_note?.[value] : undefined}
					<div class="field">
						<label for={`anki-${id}-${name}`}>{label}</label>
						{@render fieldSelect(name, mapping, field, id)}
						{#if example !== undefined}<span class="example" title={example}
								>{preview(example)}</span
							>{/if}
					</div>
				{/each}
			</div>
		</section>
		<section aria-labelledby="anki-media-heading">
			<h4 id="anki-media-heading">Save local media to Anki</h4>
			<p class="hint">
				When you mine from a local video, Yomine saves the sentence audio, the screenshot, and the
				episode with a link to it on TMDB here.
			</p>
			<div class="media-rows">
				{#each [['sentence_audio_field', 'Sentence audio', 'sentence-audio'], ['picture_field', 'Picture', 'picture'], ['source_field', 'Source', 'source']] as [key, label, id]}
					<label for={`anki-${id}-${name}`}>{label}</label>
					{@render fieldSelect(name, mapping, key as keyof ipc.FieldMapping, id)}
				{/each}
			</div>
			{#if sample?.sample_note && (mapping.sentence_audio_field || mapping.picture_field)}<button
					type="button"
					class="link disclosure"
					aria-expanded={showPreview}
					aria-controls="anki-media-preview"
					onclick={() => {
						showPreview = !showPreview;
						if (!showPreview) stopAudio();
					}}
					><span aria-hidden="true">{showPreview ? '▾' : '▸'}</span>
					{showPreview ? 'Hide preview' : 'Show preview'}</button
				>
				{#if showPreview}
					{@const note = sample.sample_note}
					{@const sound = soundFile(
						(mapping.sentence_audio_field && note[mapping.sentence_audio_field]) || ''
					)}
					{@const image = imageFile((mapping.picture_field && note[mapping.picture_field]) || '')}
					<div id="anki-media-preview" class="preview">
						{#if image}{#await mediaUri(image)}<span class="frame"></span>{:then uri}{#if uri}<img
										class="frame"
										src={uri}
										alt="Sample note's screenshot"
										title={image}
									/>{:else}<span class="hint">Could not load the sample picture</span
									>{/if}{/await}{:else}<span class="hint">No picture in the sample note</span>{/if}
						{#if sound}<button
								type="button"
								class="play"
								title={sound}
								aria-pressed={playing === sound}
								onclick={() => togglePlay(sound)}
								><span aria-hidden="true">{playing === sound ? '■' : '▶'}</span>
								{playing === sound ? 'Stop' : 'Play sample'}</button
							>{:else}<span class="hint">No sentence audio in the sample note</span>{/if}
					</div>
				{/if}
			{/if}
		</section>
		{#if sampleErrors[name]}<p class="hint">
				Could not load the sample note. Your field selections are kept.
			</p>
			<button type="button" class="reset" onclick={() => loadSample(name)}>Retry sample</button
			>{:else if sample?.sample_note === null}<p class="hint">No sample note available yet.</p>{/if}
		<div class="mapping-actions">
			<button type="button" class="reset danger" onclick={() => removeMapping(name)}
				>Remove mapping</button
			><span class="hint">Anki notes are kept.</span>
		</div>
	</div>
{/snippet}

{#snippet permissions(missing: ipc.MissingPermissions)}
	<div class="permissions" role="status">
		<p>⚠ Tsunagi's role "{missing.role}" for the app "{missing.app}" doesn't allow:</p>
		<ul>
			{#each missing.features as feature (feature.feature)}
				<li>{feature.feature} ({feature.needs})</li>
			{/each}
		</ul>
		<p>Change the role in Tsunagi's settings, then test again.</p>
	</div>
{/snippet}

{#snippet testResult(phase: Phase, incomplete = false)}
	<span
		class="test-result"
		class:ok={phase === 'ready' && !incomplete}
		class:warning={phase === 'failed' || incomplete}
		role="status"
		>{phase === 'loading'
			? 'Testing…'
			: phase === 'ready'
				? incomplete
					? '⚠ Test passed, but permissions are missing'
					: '✓ Test passed'
				: phase === 'failed'
					? '✗ Test failed'
					: ''}</span
	>
{/snippet}

<Modal
	open={$ankiModalOpen}
	title={editingModel ? `${editingModel} - Field Mapping` : 'Anki Settings'}
	width="min(760px, 94%)"
	onback={editingModel ? closeEditor : undefined}
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
			{#if editingModel && draft.anki_model_mappings[editingModel]}
				{@render editor(editingModel, draft.anki_model_mappings[editingModel])}
			{:else}
				<section aria-labelledby="anki-connections-heading">
					<h3 id="anki-connections-heading">Connections</h3>
					<div class="list">
						<div class="connection">
							<div class="connection-row">
								<strong>Anki</strong>
								<span
									class="status"
									class:ok={ankiActive.phase === 'ready' && !ankiActive.missing}
									class:warning={ankiActive.phase === 'failed' || ankiActive.missing}
									role="status">{connectionLabel(ankiActive.phase, backendLabel(ankiActive))}</span
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
							{#if ankiActive.missing}{@render permissions(ankiActive.missing)}{/if}
							{#if ankiExpanded}
								<div id="anki-connection-panel" class="panel">
									<p class="hint purpose">
										Tsunagi or AnkiConnect: reads your cards and saves mined notes.
									</p>
									<div class="test-row">
										<label for="anki-host">Host / IP</label>
										<label for="anki-port">Port</label>
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
										<input
											id="anki-port"
											type="number"
											min="1"
											max="65535"
											step="1"
											bind:value={draft.anki_connection.port}
											oninput={editConnection}
											aria-invalid={!validPort}
										/>
										<button
											type="button"
											disabled={!validHost || !validPort || ankiTest.phase === 'loading'}
											onclick={() => testAnki()}>Test connection</button
										>
										{#if ankiTest !== ankiActive}{@render testResult(
												ankiTest.phase,
												!!ankiTest.missing
											)}{/if}
									</div>
									<p id="anki-host-help" class="hint">
										Use localhost for this computer, or the address of the computer running Anki.
									</p>
									{#if !validHost}<p class="error">
											Enter a hostname or IP address without a URL or port.
										</p>{/if}
									{#if !validPort}<p class="error">Enter a whole number from 1 to 65535.</p>{/if}
									{#if ankiTest !== ankiActive && ankiTest.missing}{@render permissions(
											ankiTest.missing
										)}{/if}
									{#if ankiTest.error}<p class="error" role="status">{ankiTest.error.message}</p>
										<details>
											<summary>Technical details</summary>
											<p class="detail">{ankiTest.error.detail}</p>
										</details>{/if}
									<details class="authentication">
										<summary
											>Authentication {draft.anki_connection.api_key
												? '· Key configured'
												: '(optional)'}</summary
										>
										<div class="field">
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
													onclick={() => (showKey = !showKey)}
													>{showKey ? 'Hide' : 'Show'} key</button
												>
											</div>
										</div>
										<p id="anki-key-help" class="hint">
											Enter the API key configured in your Anki add-on, if required.
										</p>
									</details>
								</div>
							{/if}
						</div>
						<div class="connection">
							<div class="connection-row">
								<strong>Yomitan</strong>
								<span
									class="status"
									class:ok={yomitanActive.phase === 'ready'}
									class:warning={yomitanActive.phase === 'failed'}
									role="status">{connectionLabel(yomitanActive.phase, yomitanActive.version)}</span
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
									<p class="hint purpose">Builds cards with your Yomitan templates.</p>
									<div class="test-row">
										<label for="yomitan-url">URL</label>
										<input
											id="yomitan-url"
											type="url"
											bind:value={draft.yomitan_url}
											oninput={editYomitan}
										/>
										<button
											type="button"
											disabled={!validUrl(draft.yomitan_url.trim()) ||
												yomitanTest.phase === 'loading'}
											onclick={() => testYomitan()}>Test connection</button
										>
										{#if yomitanTest !== yomitanActive}{@render testResult(yomitanTest.phase)}{/if}
									</div>
									{#if yomitanTest.error}<p class="error" role="status">{yomitanTest.error}</p>{/if}
								</div>{/if}
						</div>
					</div>
					<p class="hint">Test checks the entered values. Save Settings applies changes.</p>
				</section>
				<section aria-labelledby="anki-models-heading">
					<div class="heading-row">
						<h3 id="anki-models-heading">Note types</h3>
						<span class="hint">{noteTypeCounts}</span><button
							type="button"
							class="link"
							title="Reload the note types from Anki"
							disabled={!validHost ||
								!validPort ||
								ankiTest.phase !== 'ready' ||
								catalogPhase === 'loading'}
							onclick={fetchModels}><span aria-hidden="true">↻</span> Refresh</button
						>
					</div>
					<p class="hint">Configure how Yomine reads your Anki notes and saves local media.</p>
					{#if catalogError}<details>
							<summary>Load error details</summary>
							<p class="detail">{catalogError}</p>
						</details>{/if}
					{#if Object.keys(draft.anki_model_mappings).length}<div class="list">
							{#each Object.keys(draft.anki_model_mappings) as name (name)}
								<div class="mapping-row">
									<strong>{name}</strong>
									<button
										id={`anki-edit-${name}`}
										type="button"
										class="quiet"
										aria-label={`Edit ${name} mapping`}
										onclick={() => editMapping(name)}>Edit ›</button
									>
								</div>
							{/each}
						</div>{:else}<p class="hint">
							No note types configured. Add one to recognize words from your Anki cards.
						</p>{/if}
					<button
						id="anki-add-model"
						type="button"
						class="quiet add-model"
						aria-expanded={adding}
						aria-controls="anki-model-picker"
						disabled={!availableModels.length}
						onclick={toggleAdding}>+ Add note type</button
					>
					{#if adding}<div id="anki-model-picker" class="model-picker" style={pickerStyle}>
							<label for="anki-model-search">Search note types</label>
							<div class="add-row">
								<input
									id="anki-model-search"
									role="combobox"
									aria-autocomplete="list"
									aria-expanded="true"
									aria-controls="anki-model-options"
									aria-activedescendant={activeModel
										? `anki-model-option-${modelIndex}`
										: undefined}
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
							<div
								id="anki-model-options"
								class="model-options"
								role="listbox"
								aria-label="Note types"
							>
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
							{#if !filteredModels.length}<p class="hint" role="status">
									No matching note types.
								</p>{/if}
						</div>{/if}
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
				<section aria-labelledby="anki-interval-heading">
					<h3 id="anki-interval-heading">Comprehension estimate</h3>
					<div class="input-row interval">
						<label for="anki-interval">Review interval for full credit</label>
						<input
							id="anki-interval"
							type="number"
							min="1"
							max="365"
							step="1"
							bind:value={draft.anki_interval}
						/><span class="hint">days</span>
					</div>
					<p class="hint">
						Cards with review intervals of at least {Number.isInteger(draft.anki_interval)
							? draft.anki_interval
							: 'that many'} days count as fully known. Cards with shorter intervals count partially.
					</p>
				</section>
			{/if}
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
			restore={editingModel
				? {
						label: 'Reset Mapping',
						title: `Set ${editingModel}'s fields to the ones Yomine detects. Other note types and settings are kept.`,
						run: () => editingModel && resetMapping(editingModel)
					}
				: {
						label: 'Reset Anki Settings',
						title:
							'Restore connection settings, the Yomitan URL, all note type mappings, and the comprehension interval to defaults.',
						run: restoreDefault
					}}
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
	.heading-row,
	.mapping-row,
	.mapping-actions,
	.removed,
	.input-row,
	.add-row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}
	.mapping-row {
		justify-content: space-between;
	}
	.heading-row {
		flex-wrap: wrap;
		align-items: baseline;
	}
	.list {
		border: 1px solid var(--border);
		border-radius: var(--radius);
		margin-top: 0.5rem;
	}
	.list > * + * {
		border-top: 1px solid var(--border);
	}
	.connection-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto 6.5rem;
		align-items: center;
		gap: 0.5rem;
		padding: 0.3rem 0.75rem;
	}
	strong {
		font-size: 0.88rem;
	}
	.mapping-row {
		padding: 0.4rem 0.75rem;
	}
	.mapping-row strong {
		min-width: 0;
		overflow-wrap: anywhere;
		color: var(--info);
	}
	.add-model {
		margin-top: 0.3rem;
	}
	.panel {
		margin: 0 0.75rem;
		padding: 0.7rem 0;
		border-top: 1px solid var(--border);
	}
	.input-row {
		min-width: 0;
	}
	.input-row input:not([type='number']) {
		flex: 1;
		width: 0;
	}
	.purpose {
		margin: 0 0 0.6rem;
	}
	.test-row {
		display: grid;
		grid-template-columns: minmax(10rem, 1fr) repeat(3, auto);
		align-items: center;
		gap: 0.35rem 0.6rem;
	}
	.test-row label {
		grid-row: 1;
	}
	.test-row > :not(label) {
		grid-row: 2;
	}
	.test-result {
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.link {
		padding: 0;
		font-size: 0.8rem;
		color: var(--accent);
		background: none;
		border: none;
	}
	.link:hover:not(:disabled) {
		background: none;
		text-decoration: underline;
	}
	.link:disabled {
		opacity: 0.5;
		cursor: default;
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
	.authentication .field {
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
	.permissions {
		margin: 0 0.75rem;
		padding-bottom: 0.65rem;
		color: var(--warning);
		font-size: 0.85rem;
	}
	.permissions p {
		margin: 0;
	}
	.permissions ul {
		margin: 0.25rem 0;
		padding-left: 1.25rem;
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
		white-space: nowrap;
	}
	.play span {
		font-size: 0.7rem;
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
	.editor {
		display: grid;
		gap: 0.75rem;
	}
	.editor section {
		margin-top: 0.5rem;
	}
	h4 {
		margin: 0;
		font-size: 0.85rem;
		font-weight: 600;
	}
	h4 + .hint {
		margin-bottom: 0.6rem;
	}
	.media-rows {
		display: grid;
		grid-template-columns: 7rem minmax(0, 15rem);
		align-items: center;
		gap: 0.65rem 0.9rem;
	}
	.disclosure {
		margin-top: 0.75rem;
	}
	.preview {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.9rem;
		margin-top: 0.5rem;
		padding: 0.6rem;
		background: var(--bg-raised);
		border-radius: var(--radius);
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
	.interval {
		margin-top: 0.5rem;
	}
	.mapping-actions {
		margin-top: 0.4rem;
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
		}
		.connection-row button {
			grid-column: 2;
			grid-row: 1 / 3;
		}
		.mapping-fields,
		.media-rows {
			grid-template-columns: 1fr;
		}
		.test-row {
			grid-template-columns: minmax(0, 1fr) auto;
		}
		.test-row label,
		.test-row > :not(label) {
			grid-row: auto;
		}
		.input-row {
			flex-wrap: wrap;
		}
		.input-row input:not([type='number']) {
			min-width: 8rem;
		}
	}
</style>
