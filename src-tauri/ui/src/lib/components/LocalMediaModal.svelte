<script lang="ts">
	import { tick, untrack } from 'svelte';
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
	let configuring = $state(false);
	let ffmpegChecked = '';
	let progress = $state<string | null>(null);
	let downloadError = $state<string | null>(null);

	$effect(() => {
		if (!$localMediaModalOpen) return;
		untrack(() => {
			downloadError = null;
			configuring = false;
		});
		// After the draft has loaded the saved path.
		void tick().then(checkFfmpeg);
	});

	async function checkFfmpeg() {
		const configured = draft.ffmpeg_path.trim();
		ffmpegChecked = configured;
		const status = await ipc.getFfmpegStatus(configured).catch(() => null);
		if (ffmpegChecked === configured) ffmpeg = status;
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
		if ($defaultSettings) form.value = fields($defaultSettings);
	}

	const detail = $derived(
		progress ??
			(ffmpeg === null
				? 'Checking…'
				: ffmpeg.path === 'ffmpeg'
					? 'Using the copy on PATH'
					: ffmpeg.path
						? ffmpeg.path
						: ffmpeg.downloadable
							? 'Download it, or enter its path under Configure.'
							: 'Install it, or enter its path under Configure.')
	);
</script>

<Modal
	open={$localMediaModalOpen}
	title="Local Media"
	width="min(560px, 94%)"
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<p class="hint">How sentence audio and screenshots are cut from videos on your computer.</p>
		<div class="card">
			<div class="card-row">
				<div>
					<strong>ffmpeg</strong>
					<p class="hint path" role="status">{detail}</p>
				</div>
				<span
					class="status"
					class:ok={!!ffmpeg?.path}
					class:warning={ffmpeg !== null && !ffmpeg.path}
					>{ffmpeg === null ? 'Checking' : ffmpeg.path ? 'Found' : 'Not found'}</span
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
					<div class="rows">
						<label for="ffmpeg-path">Path</label><input
							id="ffmpeg-path"
							bind:value={draft.ffmpeg_path}
							onchange={checkFfmpeg}
							placeholder="Find automatically"
							spellcheck="false"
							autocomplete="off"
						/>
					</div>
					<p class="hint indent">Leave it empty to use ffmpeg from PATH or the downloaded copy.</p>
				</div>{/if}
		</div>
		{#if downloadError}<p class="error">Download failed: {downloadError}</p>{/if}

		<section aria-labelledby="audio-heading">
			<h3 id="audio-heading">Sentence audio</h3>
			<div class="rows">
				<label for="media-audio">Format</label>
				<select id="media-audio" bind:value={draft.media_format.audio}
					><option value="mp3">MP3</option><option value="opus">Opus</option></select
				>
				<span class="label" id="padding-label">Padding</span>
				<div class="inline" role="group" aria-labelledby="padding-label">
					<label class="sub" for="media-pad_start_ms">Before</label>
					<span class="unit"
						><input
							id="media-pad_start_ms"
							type="number"
							min="0"
							max="10000"
							step="1"
							bind:value={draft.media_format.pad_start_ms}
						/><span aria-hidden="true">ms</span></span
					>
					<label class="sub" for="media-pad_end_ms">After</label>
					<span class="unit"
						><input
							id="media-pad_end_ms"
							type="number"
							min="0"
							max="10000"
							step="1"
							bind:value={draft.media_format.pad_end_ms}
						/><span aria-hidden="true">ms</span></span
					>
				</div>
			</div>
		</section>

		<section aria-labelledby="screenshot-heading">
			<h3 id="screenshot-heading">Screenshot</h3>
			<div class="rows">
				<label for="media-image">Format</label>
				<select id="media-image" bind:value={draft.media_format.image}
					><option value="jpeg">JPEG</option><option value="png">PNG</option></select
				>
				<span class="label" id="size-label">Max size</span>
				<div class="inline" role="group" aria-labelledby="size-label">
					<span class="unit"
						><input
							id="media-max_width"
							type="number"
							min="0"
							max="10000"
							step="1"
							aria-label="Maximum width"
							bind:value={draft.media_format.max_width}
						/><span aria-hidden="true">px</span></span
					><span class="times" aria-hidden="true">×</span><span class="unit"
						><input
							id="media-max_height"
							type="number"
							min="0"
							max="10000"
							step="1"
							aria-label="Maximum height"
							bind:value={draft.media_format.max_height}
						/><span aria-hidden="true">px</span></span
					>
				</div>
				<p class="hint note">0 keeps the video's width or height.</p>
			</div>
		</section>
		{#if !valid}<p class="error">Enter padding and sizes as whole numbers from 0 to 10000.</p>{/if}
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			invalid={!valid}
			onsave={save}
			oncancel={form.revert}
			onrestore={restoreDefault}
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
		margin: 0 0 0.6rem;
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
		padding: 0.65rem 0.75rem;
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
	.rows {
		display: grid;
		grid-template-columns: 5.5rem minmax(0, 1fr);
		align-items: center;
		gap: 0.6rem 0.75rem;
	}
	.indent {
		margin-left: 6.25rem;
	}
	.rows select {
		justify-self: start;
		min-width: 9rem;
	}
	.inline {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.45rem;
	}
	label,
	.label {
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
		width: 4.5rem;
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
	.sub {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.sub:not(:first-child) {
		margin-left: 0.6rem;
	}
	.times {
		color: var(--text-muted);
	}
	.note {
		grid-column: 2;
		margin: -0.3rem 0 0;
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
	@media (max-width: 520px) {
		.card-row {
			grid-template-columns: minmax(0, 1fr) auto;
		}
		.actions {
			grid-column: 1 / -1;
		}
		.rows {
			grid-template-columns: 1fr;
		}
		.note {
			grid-column: 1;
		}
		.indent {
			margin-left: 0;
		}
	}
</style>
