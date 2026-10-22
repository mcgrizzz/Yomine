<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { open as openDialog } from '@tauri-apps/plugin-dialog';
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import * as ipc from '$lib/ipc';
	import { defaultSettings, localMediaModalOpen, saveLocalMedia, settings } from '$lib/stores';

	type Draft = Pick<ipc.SettingsData, 'media_format' | 'ffmpeg_path'>;
	const fields = (s: ipc.SettingsData): Draft => ({
		media_format: { ...s.media_format },
		ffmpeg_path: s.ffmpeg_path
	});
	const form = settingsDraft<Draft>({
		open: localMediaModalOpen,
		initial: {
			media_format: {
				audio: 'mp3',
				image: 'jpeg',
				pad_start_ms: 0,
				pad_end_ms: 500,
				max_width: 0,
				max_height: 0
			},
			ffmpeg_path: ''
		},
		load: () => {
			const s = $settings ?? $defaultSettings;
			return s ? fields(s) : undefined;
		}
	});
	const draft = $derived(form.value);

	const numbers = ['pad_start_ms', 'pad_end_ms', 'max_width', 'max_height'] as const;
	const valid = $derived(
		numbers.every((key) => {
			const value = draft.media_format[key];
			return Number.isInteger(value) && value >= 0 && value <= 10000;
		})
	);

	let ffmpeg = $state<ipc.FfmpegStatus | null>(null);
	/** The ffmpeg found without the custom path; undefined while checking. */
	let detected = $state<string | null | undefined>(undefined);
	let configuring = $state(false);
	/** Custom size chosen while both limits are still 0. */
	let customSize = $state(false);
	const sizeLimited = $derived(
		customSize || draft.media_format.max_width > 0 || draft.media_format.max_height > 0
	);
	let ffmpegChecked = '';
	let progress = $state<string | null>(null);
	let downloadError = $state<string | null>(null);

	$effect(() => {
		if (!$localMediaModalOpen) return;
		untrack(() => {
			downloadError = null;
			configuring = false;
			customSize = false;
		});
		// After the draft has loaded the saved path.
		void tick().then(checkFfmpeg);
	});

	async function checkFfmpeg() {
		const configured = draft.ffmpeg_path.trim();
		ffmpegChecked = configured;
		const status = (path: string) => ipc.getFfmpegStatus(path).catch(() => null);
		const [withCustom, automatic] = await Promise.all([
			status(configured),
			configured ? status('') : null
		]);
		if (ffmpegChecked !== configured) return;
		ffmpeg = withCustom;
		detected = (configured ? automatic : withCustom)?.path ?? null;
	}

	async function browse() {
		const picked = await openDialog({ title: 'Choose the ffmpeg executable' });
		if (typeof picked !== 'string') return;
		draft.ffmpeg_path = picked;
		await checkFfmpeg();
	}

	function chooseSize(custom: boolean) {
		customSize = custom;
		if (!custom) {
			draft.media_format.max_width = 0;
			draft.media_format.max_height = 0;
		}
	}

	function revert() {
		customSize = false;
		form.revert();
	}

	async function downloadFfmpeg() {
		downloadError = null;
		progress = 'Starting download…';
		try {
			await ipc.installFfmpeg((msg) => (progress = msg.message || progress));
			await checkFfmpeg();
		} catch (error) {
			downloadError = String(error);
		} finally {
			progress = null;
		}
	}

	async function save() {
		if (await saveLocalMedia(draft.media_format, draft.ffmpeg_path.trim()))
			localMediaModalOpen.set(false);
	}

	function restoreDefault() {
		customSize = false;
		if ($defaultSettings) form.value = fields($defaultSettings);
	}

	const detectedText = $derived(
		detected === undefined
			? 'Looking for ffmpeg…'
			: detected === 'ffmpeg'
				? 'Detected automatically: the copy on PATH'
				: detected
					? `Detected automatically: ${detected}`
					: ffmpeg?.downloadable
						? 'Not detected automatically. Download it, or choose the executable.'
						: 'Not detected automatically. Install it, or choose the executable.'
	);
	const customIgnored = $derived(
		!!ffmpeg && !!draft.ffmpeg_path.trim() && ffmpeg.path !== draft.ffmpeg_path.trim()
	);
</script>

{#snippet amount(key: 'pad_start_ms' | 'pad_end_ms', unit: string)}
	<span class="unit"
		><input
			id={`media-${key}`}
			type="number"
			min="0"
			max="10000"
			step="1"
			bind:value={draft.media_format[key]}
		/><span aria-hidden="true">{unit}</span></span
	>
{/snippet}

{#snippet limit(key: 'max_width' | 'max_height', label: string)}
	<div class="field">
		<label for={`media-${key}`}>{label}</label>
		<span class="unit"
			><input
				id={`media-${key}`}
				type="number"
				min="0"
				max="10000"
				step="1"
				placeholder="Original"
				value={draft.media_format[key] || ''}
				oninput={(e) =>
					(draft.media_format[key] =
						e.currentTarget.value === '' ? 0 : e.currentTarget.valueAsNumber)}
			/><span aria-hidden="true">px</span></span
		>
	</div>
{/snippet}

<Modal
	open={$localMediaModalOpen}
	title="Local media"
	width="min(600px, 94%)"
	page
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<p class="hint">Configure sentence audio and screenshots from local videos.</p>
		<div class="card">
			<div class="card-row">
				<strong>ffmpeg</strong>
				<span
					class="status"
					class:ok={!!ffmpeg?.path}
					class:warning={ffmpeg !== null && !ffmpeg.path}
					title={ffmpeg?.path ?? undefined}
					role="status"
					>{progress ?? (ffmpeg === null ? 'Checking…' : ffmpeg.path ? 'Found' : 'Not found')}</span
				>
				<div class="actions">
					{#if ffmpeg && !ffmpeg.path && ffmpeg.downloadable}<button
							type="button"
							disabled={progress !== null}
							onclick={downloadFfmpeg}>Download</button
						>{/if}
					<button
						type="button"
						class="quiet"
						aria-expanded={configuring}
						aria-controls="ffmpeg-panel"
						onclick={() => (configuring = !configuring)}
						>{configuring ? 'Collapse' : 'Configure'}</button
					>
				</div>
			</div>
			{#if configuring}<div id="ffmpeg-panel" class="panel">
					<div class="field">
						<label for="ffmpeg-path">Custom executable (optional)</label>
						<div class="input-row">
							<input
								id="ffmpeg-path"
								bind:value={draft.ffmpeg_path}
								onchange={checkFfmpeg}
								placeholder="Detect automatically"
								spellcheck="false"
								autocomplete="off"
							/><button type="button" onclick={browse}>Browse…</button>
						</div>
					</div>
					<p class="hint path">{detectedText}</p>
					{#if customIgnored}<p class="error">
							That file doesn't run as ffmpeg{detected ? ', so Yomine uses the detected one' : ''}.
						</p>{/if}
				</div>{/if}
		</div>
		{#if downloadError}<p class="error">Download failed: {downloadError}</p>{/if}

		<div class="columns">
			<section aria-labelledby="audio-heading">
				<h3 id="audio-heading">Sentence audio</h3>
				<div class="field">
					<label for="media-audio">Format</label>
					<select id="media-audio" bind:value={draft.media_format.audio}
						><option value="mp3">MP3</option><option value="opus">Opus</option></select
					>
				</div>
				<div class="pair">
					<div class="field">
						<label for="media-pad_start_ms">Before sentence</label>
						{@render amount('pad_start_ms', 'ms')}
					</div>
					<div class="field">
						<label for="media-pad_end_ms">After sentence</label>
						{@render amount('pad_end_ms', 'ms')}
					</div>
				</div>
			</section>

			<section aria-labelledby="screenshot-heading">
				<h3 id="screenshot-heading">Screenshot</h3>
				<div class="field">
					<label for="media-image">Format</label>
					<select id="media-image" bind:value={draft.media_format.image}
						><option value="jpeg">JPEG</option><option value="png">PNG</option></select
					>
				</div>
				<div class="field">
					<label for="media-size">Size</label>
					<select
						id="media-size"
						value={sizeLimited ? 'custom' : 'original'}
						onchange={(e) => chooseSize(e.currentTarget.value === 'custom')}
						><option value="original">Original size</option><option value="custom"
							>Custom size</option
						></select
					>
				</div>
				{#if sizeLimited}<div class="pair">
						{@render limit('max_width', 'Max width')}
						{@render limit('max_height', 'Max height')}
					</div>
					<p class="hint">Larger screenshots shrink to fit and keep their shape.</p>{/if}
			</section>
		</div>
		{#if !valid}<p class="error">Enter times and sizes as whole numbers from 0 to 10000.</p>{/if}
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			invalid={!valid}
			onsave={save}
			oncancel={revert}
			restore={{
				label: 'Reset Local Media Settings',
				title:
					'Restore the ffmpeg path and the sentence audio and screenshot settings to their defaults.',
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
	h3 {
		margin: 0;
		font-size: 0.95rem;
	}
	p {
		margin: 0.3rem 0 0;
	}
	.hint {
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.45;
	}
	strong {
		font-size: 0.88rem;
	}
	.card {
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.card-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.75rem;
		padding: 0.3rem 0.75rem;
	}
	.path {
		overflow-wrap: anywhere;
	}
	.actions {
		display: flex;
		gap: 0.4rem;
	}
	.panel {
		margin: 0 0.75rem;
		padding: 0.7rem 0;
		border-top: 1px solid var(--border);
	}
	.status {
		font-size: 0.78rem;
		color: var(--text-muted);
		white-space: nowrap;
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
	.columns {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
		gap: 1.25rem 2rem;
	}
	section {
		display: grid;
		align-content: start;
		gap: 0.75rem;
	}
	.field {
		display: grid;
		justify-items: start;
		gap: 0.3rem;
	}
	.pair {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
	}
	.input-row {
		display: flex;
		gap: 0.5rem;
		width: 100%;
	}
	.input-row input {
		flex: 1;
	}
	.panel .field {
		justify-items: stretch;
	}
	/* Wide enough for "Original size", so every dropdown matches. */
	section select {
		width: 11rem;
	}
	section select,
	.unit {
		box-sizing: border-box;
		height: 2.2rem;
	}
	label {
		font-size: 0.85rem;
	}
	input,
	select {
		min-width: 0;
		padding: 0.4rem 0.5rem;
		background: var(--bg-raised);
		color: var(--text);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font: inherit;
	}
	.unit {
		display: inline-flex;
		align-items: center;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.unit:focus-within {
		border-color: var(--accent);
	}
	.unit:has(input:focus-visible) {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
	/* Outranks app.css's input:not(...):not(...):not(...) surface. */
	.unit input[type='number'] {
		width: 5.5rem;
		height: 100%;
		padding-block: 0;
		padding-right: 0.25rem;
		border: 0;
		background: transparent;
	}
	.unit input[type='number']:focus-visible {
		outline: none;
	}
	.unit span {
		padding: 0 0.6rem 0 0.1rem;
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.quiet {
		background: transparent;
		border-color: transparent;
	}
	.error {
		color: var(--danger);
		font-size: 0.85rem;
		overflow-wrap: anywhere;
	}
</style>
