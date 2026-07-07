<script lang="ts">
	// Term-table controls (T037, US4): search, POS filter, and a frequency range —
	// all client-side, writing the control stores that `visibleTerms` derives
	// from. Mirrors egui's controls row (`table/controls.rs`): search bar +
	// dual-thumb log frequency slider with Min/Max numeric fields and the "?"
	// unknown toggle. Sorting lives in the table's column headers (T061, like
	// egui); the POS button opens the POS modal (T043) — single surface for POS,
	// a deliberate deviation from egui's header popover (maintainer, 2026-06-11).
	import {
		tableSearch,
		posEnabled,
		posCatalog,
		freqFilter,
		visibleTerms,
		fileResult,
		openPosModal
	} from '$lib/stores';
	import DualSlider from './DualSlider.svelte';

	const posTotal = $derived($posCatalog.length);
	const posOn = $derived($posCatalog.filter((p) => $posEnabled[p.key] !== false).length);

	// Slider drags move both ends live; the numeric fields commit on change
	// (egui's DragValues), clamped to the bounds and to each other.
	function setRange(min: number, max: number) {
		freqFilter.update((f) => (f ? { ...f, min, max } : f));
	}
	function commitMin(v: number) {
		freqFilter.update((f) => {
			if (!f || Number.isNaN(v)) return f;
			const min = Math.min(Math.max(v, f.lo), f.hi);
			return { ...f, min, max: Math.max(f.max, min) };
		});
	}
	function commitMax(v: number) {
		freqFilter.update((f) => {
			if (!f || Number.isNaN(v)) return f;
			const max = Math.min(Math.max(v, f.lo), f.hi);
			return { ...f, max, min: Math.min(f.min, max) };
		});
	}
	function setUnknown(on: boolean) {
		freqFilter.update((f) => (f ? { ...f, includeUnknown: on } : f));
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

	{#if $freqFilter && $freqFilter.hi > $freqFilter.lo}
		<div class="group freq">
			<span class="lbl">Freq</span>
			<DualSlider
				lo={$freqFilter.lo}
				hi={$freqFilter.hi}
				min={$freqFilter.min}
				max={$freqFilter.max}
				onchange={setRange}
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

	<span class="spacer"></span>
	<span class="count">{$visibleTerms.length} / {$fileResult?.terms.length ?? 0} shown</span>
</div>

<style>
	.controls {
		display: flex;
		flex-wrap: wrap;
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
		color: var(--comment);
		text-transform: uppercase;
		font-size: 0.7rem;
		letter-spacing: 0.03em;
	}
	.pos {
		cursor: pointer;
		padding: 0.3rem 0.6rem;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		color: var(--fg);
	}
	/* Min/Max numeric bounds beside the slider (egui's DragValues). */
	.bound {
		width: 5.5rem;
		padding: 0.2rem 0.35rem;
		font-variant-numeric: tabular-nums;
	}
	.dash {
		color: var(--comment);
	}
	.unknown {
		display: flex;
		align-items: center;
		gap: 0.2rem;
		color: var(--comment);
		cursor: pointer;
	}
	.no-freq {
		color: var(--red);
	}
	.spacer {
		flex: 1;
	}
	.count {
		color: var(--comment);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
</style>
