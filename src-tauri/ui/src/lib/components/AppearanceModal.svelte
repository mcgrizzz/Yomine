<script lang="ts">
	// Staged, but live-previews while adjusting so the user can judge readability;
	// Discard Changes/✕/backdrop revert the preview to the saved value.
	import { settingsDraft } from '$lib/settingsDraft.svelte';
	import Modal from './Modal.svelte';
	import SettingsFooter from './SettingsFooter.svelte';
	import type { SegmentKnowledge, SettingsData } from '$lib/ipc';
	import { defaultSettings, settings, appearanceModalOpen, saveAppearance } from '$lib/stores';

	const MIN_PCT = 75;
	const MAX_PCT = 150;
	const STEP = 5;
	const DEF_MIN_PCT = 50;

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

	const fields = (s: SettingsData) => ({
		pct: Math.round(s.font_scale * 100),
		defPct: Math.round(s.definition_scale * 100),
		coloring: s.sentence_coloring,
		toggles: { ...s.sentence_underlines }
	});
	const form = settingsDraft<ReturnType<typeof fields>>({
		open: appearanceModalOpen,
		initial: {
			pct: 100,
			defPct: 100,
			coloring: 'knowledge',
			toggles: { unknown: true, new: true, young: true, mature: true }
		},
		load: () => {
			const s = $settings ?? $defaultSettings;
			return s ? fields(s) : undefined;
		},
		// Closing without saving discards the preview.
		close: () => {
			applyZoom(form.saved.pct);
			appearanceModalOpen.set(false);
		}
	});
	const draft = $derived(form.value);

	// Live preview: mirror what the root layout does with the saved setting.
	function applyZoom(pct: number) {
		document.documentElement.style.setProperty('zoom', String(pct / 100));
	}
	$effect(() => {
		if ($appearanceModalOpen) applyZoom(draft.pct);
	});

	function step(delta: number) {
		draft.pct = Math.min(MAX_PCT, Math.max(MIN_PCT, draft.pct + delta));
	}

	function stepDef(delta: number) {
		draft.defPct = Math.min(MAX_PCT, Math.max(DEF_MIN_PCT, draft.defPct + delta));
	}

	async function save() {
		const { pct, defPct, coloring, toggles } = draft;
		if (!(await saveAppearance(pct / 100, defPct / 100, coloring, toggles))) return;
		form.commit();
		appearanceModalOpen.set(false);
	}

	function restoreDefault() {
		if ($defaultSettings) form.value = fields($defaultSettings);
	}
</script>

<Modal
	open={$appearanceModalOpen}
	title="Appearance"
	width="min(600px, 94%)"
	onclose={form.request}
	oninteract={form.disarm}
>
	<div class="body">
		<div class="group">
			<label for="ui-scale">UI scale</label>
			<div class="scale-row">
				<button
					class="step"
					aria-label="Decrease scale"
					disabled={draft.pct <= MIN_PCT}
					onclick={() => step(-STEP)}>−</button
				>
				<input
					id="ui-scale"
					type="range"
					min={MIN_PCT}
					max={MAX_PCT}
					step={STEP}
					bind:value={draft.pct}
				/>
				<button
					class="step"
					aria-label="Increase scale"
					disabled={draft.pct >= MAX_PCT}
					onclick={() => step(STEP)}>+</button
				>
				<span class="value">{draft.pct}%</span>
			</div>
			<p class="hint">Scales the entire interface.</p>
		</div>

		<div class="group">
			<label for="definition-scale">Definition scale</label>
			<div class="scale-row">
				<button
					class="step"
					aria-label="Decrease definition scale"
					disabled={draft.defPct <= DEF_MIN_PCT}
					onclick={() => stepDef(-STEP)}>−</button
				>
				<input
					id="definition-scale"
					type="range"
					min={DEF_MIN_PCT}
					max={MAX_PCT}
					step={STEP}
					bind:value={draft.defPct}
				/>
				<button
					class="step"
					aria-label="Increase definition scale"
					disabled={draft.defPct >= MAX_PCT}
					onclick={() => stepDef(STEP)}>+</button
				>
				<span class="value">{draft.defPct}%</span>
			</div>
			<p class="hint">Relative to UI scale; applies to Shift+Hover definitions.</p>
		</div>

		<div class="group">
			<label for="sentence-coloring">Sentence marking</label>
			<select id="sentence-coloring" bind:value={draft.coloring}>
				<option value="knowledge">Knowledge underlines</option>
				<option value="none">None</option>
			</select>
			{#if draft.coloring === 'knowledge'}
				<fieldset>
					<legend>Underline these states:</legend>
					<div class="underline-toggles">
						{#each STATES as s (s)}
							<label class="state-toggle">
								<input type="checkbox" bind:checked={draft.toggles[s]} />
								<span style="border-bottom: 2.5px solid {STATE_COLORS[s]}">{STATE_LABELS[s]}</span>
							</label>
						{/each}
					</div>
				</fieldset>
			{/if}
		</div>

		<div class="group">
			<span class="label">Table columns</span>
			<p class="hint">Right-click the table header to reorder or hide columns.</p>
		</div>
	</div>

	{#snippet footer()}
		<SettingsFooter
			{form}
			onsave={save}
			oncancel={form.revert}
			restore={{
				label: 'Reset Appearance Settings',
				title: 'Restore UI scale, definition scale and sentence marking to their defaults.',
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
	.group {
		display: grid;
		justify-items: start;
		gap: 0.35rem;
	}
	.group > label,
	.label {
		font-size: 0.85rem;
	}
	.scale-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		justify-self: stretch;
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
	select {
		min-width: 14rem;
	}
	fieldset {
		margin: 0.35rem 0 0;
		padding: 0;
		border: 0;
	}
	legend {
		margin-bottom: 0.35rem;
		padding: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.underline-toggles {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem 1rem;
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
		font-size: 0.85rem;
		color: var(--text-muted);
	}
</style>
