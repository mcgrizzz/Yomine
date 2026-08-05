<script lang="ts">
	// Staged, but live-previews while adjusting so the user can judge readability;
	// Cancel/✕/backdrop revert the preview to the saved value.
	import { untrack } from 'svelte';
	import type { SegmentKnowledge, SentenceColoring, UnderlineToggles } from '$lib/ipc';
	import {
		settings,
		appearanceModalOpen,
		setDefinitionScale,
		setFontScale,
		setSentenceColoring,
		setSentenceUnderlines
	} from '$lib/stores';

	/** `default_font_scale()` (core/settings.rs), as a percentage. */
	const DEFAULT_PCT = 100;
	const MIN_PCT = 75;
	const MAX_PCT = 150;
	const STEP = 5;
	const DEF_MIN_PCT = 50;

	let tempPct = $state(DEFAULT_PCT);
	let originalPct = $state(DEFAULT_PCT);

	let tempDefPct = $state(DEFAULT_PCT);
	let originalDefPct = $state(DEFAULT_PCT);

	const DEFAULT_COLORING: SentenceColoring = 'knowledge';
	let tempColoring = $state<SentenceColoring>(DEFAULT_COLORING);
	let originalColoring = $state<SentenceColoring>(DEFAULT_COLORING);

	const STATES: SegmentKnowledge[] = ['unknown', 'new', 'young', 'mature'];
	const STATE_LABELS: Record<SegmentKnowledge, string> = {
		unknown: 'Not in Anki',
		new: 'New',
		young: 'Young',
		mature: 'Mature'
	};
	const STATE_COLORS: Record<SegmentKnowledge, string> = {
		unknown: 'var(--know-unknown)',
		new: 'var(--know-new)',
		young: 'var(--know-young)',
		mature: 'var(--know-mature)'
	};
	const DEFAULT_TOGGLES: UnderlineToggles = { unknown: true, new: true, young: true, mature: true };
	let tempToggles = $state<UnderlineToggles>({ ...DEFAULT_TOGGLES });
	let originalToggles = $state<UnderlineToggles>({ ...DEFAULT_TOGGLES });

	// Hydrate from the settings mirror each time the modal opens; untrack so
	// settings changes while open don't clobber the staged value.
	$effect(() => {
		if ($appearanceModalOpen) untrack(hydrate);
	});

	function hydrate() {
		const pct = Math.round(($settings?.font_scale ?? 1) * 100);
		tempPct = pct;
		originalPct = pct;
		const defPct = Math.round(($settings?.definition_scale ?? 1) * 100);
		tempDefPct = defPct;
		originalDefPct = defPct;
		const coloring = $settings?.sentence_coloring ?? DEFAULT_COLORING;
		tempColoring = coloring;
		originalColoring = coloring;
		const toggles = { ...DEFAULT_TOGGLES, ...$settings?.sentence_underlines };
		tempToggles = { ...toggles };
		originalToggles = { ...toggles };
	}

	// Live preview: mirror what the root layout does with the saved setting.
	function applyZoom(pct: number) {
		document.documentElement.style.setProperty('zoom', String(pct / 100));
	}
	$effect(() => {
		if ($appearanceModalOpen) applyZoom(tempPct);
	});

	const togglesDirty = $derived(STATES.some((s) => tempToggles[s] !== originalToggles[s]));
	const dirty = $derived(
		tempPct !== originalPct ||
			tempDefPct !== originalDefPct ||
			tempColoring !== originalColoring ||
			togglesDirty
	);

	function step(delta: number) {
		tempPct = Math.min(MAX_PCT, Math.max(MIN_PCT, tempPct + delta));
	}

	function stepDef(delta: number) {
		tempDefPct = Math.min(MAX_PCT, Math.max(DEF_MIN_PCT, tempDefPct + delta));
	}

	async function save() {
		if (tempPct !== originalPct) await setFontScale(tempPct / 100);
		if (tempDefPct !== originalDefPct) await setDefinitionScale(tempDefPct / 100);
		if (tempColoring !== originalColoring) await setSentenceColoring(tempColoring);
		if (togglesDirty) await setSentenceUnderlines(tempToggles);
		originalPct = tempPct;
		originalDefPct = tempDefPct;
		originalColoring = tempColoring;
		originalToggles = { ...tempToggles };
		appearanceModalOpen.set(false);
	}

	function cancel() {
		tempPct = originalPct;
		tempDefPct = originalDefPct;
		tempColoring = originalColoring;
		tempToggles = { ...originalToggles };
	}

	// Closing without saving discards the preview.
	function close() {
		applyZoom(originalPct);
		appearanceModalOpen.set(false);
	}

	function restoreDefault() {
		tempPct = DEFAULT_PCT;
		tempDefPct = DEFAULT_PCT;
		tempColoring = DEFAULT_COLORING;
		tempToggles = { ...DEFAULT_TOGGLES };
	}
</script>

<!-- Esc closes from anywhere: the backdrop's own keydown only fires once focus
     is inside the modal, which it isn't right after opening from a menu. -->
<svelte:window onkeydown={(e) => $appearanceModalOpen && e.key === 'Escape' && close()} />

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

			<div class="scale-row">
				<label for="definition-scale">Definition scale:</label>
				<button class="step" aria-label="Decrease definition scale" onclick={() => stepDef(-STEP)}
					>−</button
				>
				<input
					id="definition-scale"
					type="range"
					min={DEF_MIN_PCT}
					max={MAX_PCT}
					step={STEP}
					bind:value={tempDefPct}
				/>
				<button class="step" aria-label="Increase definition scale" onclick={() => stepDef(STEP)}
					>+</button
				>
				<span class="value">{tempDefPct}%</span>
			</div>
			<p class="hint">Scales the Shift+Hover definition popover, on top of the UI scale.</p>

			<div class="coloring-row">
				<label for="sentence-coloring">Sentence marking:</label>
				<select id="sentence-coloring" bind:value={tempColoring}>
					<option value="knowledge">Knowledge underlines</option>
					<option value="none">None</option>
				</select>
			</div>
			{#if tempColoring === 'knowledge'}
				<div class="underline-toggles">
					{#each STATES as s (s)}
						<label class="state-toggle">
							<input type="checkbox" bind:checked={tempToggles[s]} />
							<span style="border-bottom: 2.5px solid {STATE_COLORS[s]}">{STATE_LABELS[s]}</span>
						</label>
					{/each}
				</div>
				<p class="hint">Underlines words by Anki state; untick a state to hide it.</p>
			{/if}

			<p class="hint">Table columns: right-click the term-table header to reorder or hide.</p>

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
		background: color-mix(in srgb, var(--bg-deep) 70%, transparent);
		z-index: 50;
	}
	.dialog {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		width: min(420px, 92%);
		padding-bottom: 0.75rem;
		background: var(--bg-panel);
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
		color: var(--accent);
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
		accent-color: var(--accent);
	}
	.step {
		padding: 0.1rem 0.5rem;
		font-size: 0.95rem;
		line-height: 1.2;
	}
	.coloring-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.coloring-row select {
		flex: 1;
	}
	.underline-toggles {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem 1rem;
		padding: 0 1rem;
	}
	.state-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		cursor: pointer;
	}
	.state-toggle span {
		padding-bottom: 1px;
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
		color: var(--text-muted);
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
</style>
