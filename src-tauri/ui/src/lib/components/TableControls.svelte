<script lang="ts">
	// Term-table controls (T037, US4): search, sort, POS filter, and a frequency
	// range — all client-side, writing the control stores that `visibleTerms`
	// derives from. Mirrors egui's controls row; the POS button opens the POS
	// modal (T043) — single surface for POS, a deliberate deviation from egui's
	// separate header popover (maintainer decision, 2026-06-11).
	import {
		tableSearch,
		tableSort,
		posEnabled,
		posCatalog,
		freqFilter,
		visibleTerms,
		fileResult,
		openPosModal
	} from '$lib/stores';
	import { SORT_FIELDS, defaultDir, type SortField } from '$lib/table';

	// Picking a column resets to that column's natural direction (egui parity);
	// the arrow button flips it.
	function setField(field: SortField) {
		tableSort.set({ field, dir: defaultDir(field) });
	}
	function toggleDir() {
		tableSort.update((s) => ({ ...s, dir: s.dir === 'asc' ? 'desc' : 'asc' }));
	}

	const posTotal = $derived($posCatalog.length);
	const posOn = $derived($posCatalog.filter((p) => $posEnabled[p.key] !== false).length);

	function setFreqMin(v: number) {
		freqFilter.update((f) => (f ? { ...f, min: Math.min(v, f.max) } : f));
	}
	function setFreqMax(v: number) {
		freqFilter.update((f) => (f ? { ...f, max: Math.max(v, f.min) } : f));
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

	<div class="group">
		<span class="lbl">Sort</span>
		<select value={$tableSort.field} onchange={(e) => setField(e.currentTarget.value as SortField)}>
			{#each SORT_FIELDS as f (f.value)}
				<option value={f.value}>{f.label}</option>
			{/each}
		</select>
		<button class="dir" onclick={toggleDir} title="Toggle sort direction">
			{$tableSort.dir === 'asc' ? '▲' : '▼'}
		</button>
	</div>

	<button class="pos" onclick={openPosModal} title="Edit part-of-speech filters">
		POS ({posOn}/{posTotal})
	</button>

	{#if $freqFilter && $freqFilter.hi > $freqFilter.lo}
		<div class="group freq">
			<span class="lbl">Freq</span>
			<input
				type="range"
				min={$freqFilter.lo}
				max={$freqFilter.hi}
				value={$freqFilter.min}
				oninput={(e) => setFreqMin(e.currentTarget.valueAsNumber)}
				aria-label="Minimum frequency"
			/>
			<input
				type="range"
				min={$freqFilter.lo}
				max={$freqFilter.hi}
				value={$freqFilter.max}
				oninput={(e) => setFreqMax(e.currentTarget.valueAsNumber)}
				aria-label="Maximum frequency"
			/>
			<span class="range-val">{$freqFilter.min}–{$freqFilter.max}</span>
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
	.dir {
		padding: 0.2rem 0.45rem;
		line-height: 1;
	}
	.pos {
		cursor: pointer;
		padding: 0.3rem 0.6rem;
		background: var(--bg-light);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		color: var(--fg);
	}
	.freq input[type='range'] {
		width: 6rem;
	}
	.range-val {
		color: var(--comment);
		font-variant-numeric: tabular-nums;
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
