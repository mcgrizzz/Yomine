<script lang="ts">
	// Appearance modal: whole-UI scale (settings.font_scale, applied as CSS zoom
	// by the root layout). The scale is *staged* like the WebSocket modal's port,
	// but live-previews while you adjust so you can judge readability; Save
	// persists, Cancel/✕/backdrop revert the preview to the saved value.
	import { untrack } from 'svelte';
	import { settings, appearanceModalOpen, setFontScale } from '$lib/stores';

	/** `default_font_scale()` (core/settings.rs), as a percentage. */
	const DEFAULT_PCT = 100;
	const MIN_PCT = 75;
	const MAX_PCT = 150;
	const STEP = 5;

	let tempPct = $state(DEFAULT_PCT);
	let originalPct = $state(DEFAULT_PCT);

	// Hydrate from the settings mirror each time the modal opens; untrack so
	// settings changes while open don't clobber the staged value.
	$effect(() => {
		if ($appearanceModalOpen) untrack(hydrate);
	});

	function hydrate() {
		const pct = Math.round(($settings?.font_scale ?? 1) * 100);
		tempPct = pct;
		originalPct = pct;
	}

	// Live preview: mirror what the root layout does with the saved setting.
	function applyZoom(pct: number) {
		document.documentElement.style.setProperty('zoom', String(pct / 100));
	}
	$effect(() => {
		if ($appearanceModalOpen) applyZoom(tempPct);
	});

	const dirty = $derived(tempPct !== originalPct);

	function step(delta: number) {
		tempPct = Math.min(MAX_PCT, Math.max(MIN_PCT, tempPct + delta));
	}

	async function save() {
		await setFontScale(tempPct / 100);
		originalPct = tempPct;
		appearanceModalOpen.set(false);
	}

	function cancel() {
		tempPct = originalPct;
	}

	// Closing without saving discards the preview.
	function close() {
		applyZoom(originalPct);
		appearanceModalOpen.set(false);
	}

	function restoreDefault() {
		tempPct = DEFAULT_PCT;
	}
</script>

{#if $appearanceModalOpen}
	<div
		class="backdrop"
		role="button"
		tabindex="-1"
		onclick={close}
		onkeydown={(e) => e.key === 'Escape' && close()}
	>
		<!-- Stop backdrop clicks inside the dialog from closing it. -->
		<div
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-label="Appearance settings"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
		>
			<header>
				<h2>Appearance</h2>
				<button class="close" aria-label="Close" onclick={close}>✕</button>
			</header>

			<div class="scale-row">
				<label for="ui-scale">UI scale:</label>
				<button class="step" aria-label="Decrease scale" onclick={() => step(-STEP)}>−</button>
				<input
					id="ui-scale"
					type="range"
					min={MIN_PCT}
					max={MAX_PCT}
					step={STEP}
					bind:value={tempPct}
				/>
				<button class="step" aria-label="Increase scale" onclick={() => step(STEP)}>+</button>
				<span class="value">{tempPct}%</span>
			</div>
			<p class="hint">Scales the whole interface — text, controls, and spacing.</p>

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
		width: min(420px, 92vw);
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
	.scale-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.scale-row input[type='range'] {
		flex: 1;
		accent-color: var(--cyan);
	}
	.step {
		padding: 0.1rem 0.5rem;
		font-size: 0.95rem;
		line-height: 1.2;
	}
	.value {
		min-width: 3.2rem;
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	.hint {
		margin: 0;
		padding: 0 1rem;
		font-size: 0.85rem;
		color: var(--comment);
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
