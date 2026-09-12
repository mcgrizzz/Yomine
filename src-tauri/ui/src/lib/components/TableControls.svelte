<script lang="ts">
    import { showPossibleKnownMatches } from '$lib/stores/knowledgeView';
    import { setShowPossibleKnownMatches } from '$lib/stores/settings';
	// Sorting lives in the table's column headers; POS gets a single modal here
	// (deliberate deviation from egui's per-header popovers).
	import { get } from 'svelte/store';
	import {
		tableSearch,
		posEnabled,
		posCatalog,
		freqFilter,
		jlptEnabled,
		visibleTerms,
		fileResult,
		openPosModal,
		saveJlptFilters,
		saveFreqFilter
	} from '$lib/stores';
	import { JLPT_CHIPS, type JlptChip } from '$lib/table';
	import DualSlider from './DualSlider.svelte';

	const posTotal = $derived($posCatalog.length);
	const posOn = $derived($posCatalog.filter((p) => $posEnabled[p.key] !== false).length);

	const hasJlpt = $derived($fileResult?.terms.some((t) => t.jlpt_level) ?? false);
	const jlptChips = $derived(
		JLPT_CHIPS.filter(
			(key) => $fileResult?.terms.some((t) => (t.jlpt_level ?? 'none') === key) ?? false
		)
	);
	const jlptLabel = (key: string) => (key === 'none' ? 'Non-JLPT' : key);

	let jlptAnchor = $state<JlptChip | null>(null);
	function jlptClick(e: MouseEvent, key: JlptChip) {
		const chips = jlptChips;
		if (e.shiftKey) {
			const anchorIdx = jlptAnchor ? chips.indexOf(jlptAnchor) : -1;
			const from = anchorIdx >= 0 ? anchorIdx : chips.indexOf(key);
			const to = chips.indexOf(key);
			const [lo, hi] = from <= to ? [from, to] : [to, from];
			jlptEnabled.set(Object.fromEntries(chips.map((k, i) => [k, i >= lo && i <= hi])));
			void saveJlptFilters(get(jlptEnabled));
			return;
		}
		jlptAnchor = key;
		jlptEnabled.update((m) => {
			if (e.ctrlKey || e.metaKey) return { ...m, [key]: m[key] === false };
			const isSolo = chips.every((k) => (m[k] !== false) === (k === key));
			if (isSolo) return {};
			return Object.fromEntries(chips.map((k) => [k, k === key]));
		});
		void saveJlptFilters(get(jlptEnabled));
	}

	// Slider drags apply live; the numeric fields commit on change, clamped to
	// the bounds and to each other.
	function persistFreq() {
		const f = get(freqFilter);
		if (f) void saveFreqFilter(f);
	}
	function setRange(min: number, max: number) {
		freqFilter.update((f) => (f ? { ...f, min, max } : f));
	}
	function commitMin(v: number) {
		freqFilter.update((f) => {
			if (!f || Number.isNaN(v)) return f;
			const min = Math.min(Math.max(v, f.lo), f.hi);
			return { ...f, min, max: Math.max(f.max, min) };
		});
		persistFreq();
	}
	function commitMax(v: number) {
		freqFilter.update((f) => {
			if (!f || Number.isNaN(v)) return f;
			const max = Math.min(Math.max(v, f.lo), f.hi);
			return { ...f, max, min: Math.min(f.min, max) };
		});
		persistFreq();
	}
	function setUnknown(on: boolean) {
		freqFilter.update((f) => (f ? { ...f, includeUnknown: on } : f));
		persistFreq();
	}
</script>

<div class="controls">
	<input
		class="search"
		type="search"
		bind:value={$tableSearch}
		placeholder="Search terms or sentences..."
	/>

	<button class="pos" onclick={openPosModal} title="Edit part-of-speech filters">
		POS ({posOn}/{posTotal})
	</button>

	<button
		class="possible-toggle"
		role="switch"
		aria-label="Show uncertain matches"
		aria-checked={$showPossibleKnownMatches}
		title="Include words with an uncertain Anki match in the table"
		onclick={() => void setShowPossibleKnownMatches(!$showPossibleKnownMatches)}
	>
		<span class="switch-track" class:on={$showPossibleKnownMatches} aria-hidden="true"><span class="switch-thumb"></span></span>
		Show uncertain matches
	</button>
	<div class="filters">

	{#if hasJlpt}
		<div class="group">
			<span class="lbl">JLPT</span>
			{#each jlptChips as key (key)}
				<button
					class="jlpt"
					class:off={$jlptEnabled[key] === false}
					aria-pressed={$jlptEnabled[key] !== false}
					title={`Show only ${jlptLabel(key)} — Ctrl+Click to combine, Shift+Click for a range`}
					onclick={(e) => jlptClick(e, key)}
				>
					{jlptLabel(key)}
				</button>
			{/each}
		</div>
	{/if}

	{#if $freqFilter && $freqFilter.hi > $freqFilter.lo}
		<div class="group freq">
			<span class="lbl">Freq</span>
			<DualSlider
				lo={$freqFilter.lo}
				hi={$freqFilter.hi}
				min={$freqFilter.min}
				max={$freqFilter.max}
				onchange={setRange}
				oncommit={persistFreq}
			/>
			<input
				class="bound"
				type="number"
				min={$freqFilter.lo}
				max={$freqFilter.hi}
				value={$freqFilter.min}
				onchange={(e) => commitMin(e.currentTarget.valueAsNumber)}
				aria-label="Minimum frequency"
			/>
			<span class="dash">–</span>
			<input
				class="bound"
				type="number"
				min={$freqFilter.lo}
				max={$freqFilter.hi}
				value={$freqFilter.max}
				onchange={(e) => commitMax(e.currentTarget.valueAsNumber)}
				aria-label="Maximum frequency"
			/>
			<label class="unknown" title="Include entries without frequency data">
				<input
					type="checkbox"
					checked={$freqFilter.includeUnknown}
					onchange={(e) => setUnknown(e.currentTarget.checked)}
				/>
				?
			</label>
		</div>
	{:else if $freqFilter}
		<span class="no-freq">No frequency data</span>
	{/if}


	</div>

	<span class="count">{$visibleTerms.length} / {$fileResult?.terms.length ?? 0} shown</span>
</div>

<style>
	.controls {
		display: grid;
		grid-template-columns: minmax(12rem, 1fr) auto auto auto;
		align-items: center;
		gap: 0.6rem 0.9rem;
		margin-bottom: 0.75rem;
		font-size: 0.85rem;
	}
	.search {
		flex: 1 1 14rem;
		min-width: 12rem;
		padding: 0.35rem 0.55rem;
	}
	.group {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.lbl {
		color: var(--text-muted);
		text-transform: uppercase;
		font-size: 0.7rem;
		letter-spacing: 0.03em;
	}
	.pos {
		cursor: pointer;
		padding: 0.3rem 0.6rem;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		color: var(--text);
	}
	.jlpt {
		cursor: pointer;
		padding: 0.2rem 0.45rem;
		background: var(--selection);
		border: 1px solid var(--accent);
		box-shadow: inset 0 -2px var(--accent);
		border-radius: var(--radius);
		color: var(--text);
		font-size: 0.75rem;
	}
	.jlpt.off {
		background: var(--bg-raised);
		border-color: var(--border);
		box-shadow: none;
		color: var(--text-muted);
	}
	/* Min/Max numeric bounds beside the slider (egui's DragValues). */
	.bound {
		width: 5.5rem;
		padding: 0.2rem 0.35rem;
		font-variant-numeric: tabular-nums;
	}
	.dash {
		color: var(--text-muted);
	}
	.unknown {
		display: flex;
		align-items: center;
		gap: 0.2rem;
		color: var(--text-muted);
		cursor: pointer;
	}
	.no-freq {
		color: var(--danger);
	}
	.filters {
		grid-column: 1 / -1;
		grid-row: 2;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.6rem 1.25rem;
	}
	.possible-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.35rem 0;
		border: 0;
		background: transparent;
		color: var(--text);
		font-size: 0.8rem;
		white-space: nowrap;
		cursor: pointer;
	}
	.switch-track {
		width: 2rem;
		height: 1.15rem;
		padding: 0.125rem;
		box-sizing: border-box;
		border-radius: 999px;
		background: var(--text-muted);
		transition: background 120ms ease;
	}
	.switch-track.on { background: var(--accent); }
	.switch-thumb {
		display: block;
		width: 0.9rem;
		height: 0.9rem;
		border-radius: 50%;
		background: var(--bg);
		box-shadow: 0 1px 2px #0004;
		transition: transform 120ms ease;
	}
	.switch-track.on .switch-thumb { transform: translateX(0.85rem); }
	.possible-toggle:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 3px;
		border-radius: var(--radius);
	}
	@media (prefers-reduced-motion: reduce) {
		.switch-track, .switch-thumb { transition: none; }
	}
	@media (max-width: 720px) {
		.controls { grid-template-columns: minmax(0, 1fr) auto; }
		.search { grid-column: 1 / -1; }
		.filters { grid-row: auto; }
		.count { grid-column: 1 / -1; }
	}
	.count {
		color: var(--text-muted);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
</style>
