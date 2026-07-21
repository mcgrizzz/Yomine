<script lang="ts">
	import { knowledge } from '$lib/stores';
	import type { BandStats } from '$lib/ipc';

	type Mode = 'coverage' | 'estimate';
	let mode = $state<Mode>('coverage');

	const title = $derived(mode === 'coverage' ? 'Anki Coverage' : 'Estimated Knowledge');
	const otherTitle = $derived(mode === 'coverage' ? 'Estimated Knowledge' : 'Anki Coverage');

	function toggle() {
		mode = mode === 'coverage' ? 'estimate' : 'coverage';
	}

	// coverage = raw Anki presence; estimate = graded comprehension.
	const frac = (s: BandStats) =>
		Math.min(Math.max(mode === 'coverage' ? s.coverage : s.comprehension, 0), 1);

	// Red→yellow→green WITHOUT the gray blend the comprehension text uses —
	// these bars read brighter by design.
	function barColor(pct: number): string {
		const [r, g, b] =
			pct >= 50 ? [180 * (1 - (pct - 50) / 50), 180, 60] : [180, 180 * (pct / 50), 60];
		return `rgb(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)})`;
	}

	function tip(label: string | null, s: BandStats, f: number): string {
		const got = Math.round(f * s.total);
		const pct = (f * 100).toFixed(0);
		return label ? `${label} ${got}/${s.total} ${pct}%` : `${got}/${s.total} ${pct}%`;
	}
</script>

{#if $knowledge && ($knowledge.jlpt.length > 0 || $knowledge.frequency.length > 0)}
	<div class="card">
		<div class="mode-header">
			<button class="mode-title" title={`Switch to ${otherTitle}`} onclick={toggle}>{title}</button>
			<button
				class="swap"
				title={`Switch to ${otherTitle}`}
				aria-label={`Switch to ${otherTitle}`}
				onclick={toggle}>⇄</button
			>
		</div>

		{#if $knowledge.jlpt.length > 0}
			<div class="band-row">
				{#each $knowledge.jlpt as band (band.level)}
					{@const f = frac(band.stats)}
					<div class="band" title={tip(null, band.stats, f)}>
						<div class="track" style:width="40px">
							<div
								class="fill"
								style:width={`${f * 100}%`}
								style:background={barColor(f * 100)}
							></div>
						</div>
						<span class="band-label">{band.level}</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if $knowledge.frequency.length > 0}
			<div class="band-row">
				{#each $knowledge.frequency as band (band.label)}
					{@const f = frac(band.stats)}
					<div class="band" title={tip(band.label, band.stats, f)}>
						<div class="track" style:width="28px">
							<div
								class="fill"
								style:width={`${f * 100}%`}
								style:background={barColor(f * 100)}
							></div>
						</div>
						<span class="band-label">{band.label}</span>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style>
	.card {
		display: inline-flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 4px;
		align-self: flex-start;
	}
	.mode-header {
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.mode-title {
		padding: 0;
		background: none;
		border: none;
		font-size: 14px;
		font-weight: 700;
		color: var(--text);
		cursor: pointer;
	}
	.swap {
		padding: 0;
		background: none;
		border: none;
		font-size: 13px;
		line-height: 1;
		color: var(--text-muted);
		cursor: pointer;
	}
	.band-row {
		display: flex;
		gap: 6px;
	}
	.band {
		display: flex;
		align-items: center;
		gap: 3px;
	}
	.track {
		position: relative;
		flex: none;
		height: 9px;
		background: var(--bg-deep);
		border-radius: 2px;
		overflow: hidden;
	}
	.fill {
		height: 100%;
		border-radius: 2px;
	}
	.band-label {
		font-family: ui-monospace, monospace;
		font-size: 11px;
		color: var(--text-muted);
	}
</style>
