<script lang="ts">
	import AutoLedger from './AutoLedger.svelte';
	import AutoWaiting from './AutoWaiting.svelte';
	import {
		autoAvailable,
		autoCountdown,
		autoLedger,
		autoMine,
		autoMode,
		autoSkipped,
		fileResult,
		playerBusy,
		setAutoMode,
		settings
	} from '$lib/stores';

	const limit = $derived($settings?.auto_mine.limit ?? 10);
	const tip = $derived(
		$autoMode
			? 'Auto mode is on. Click to stop mining new videos automatically.'
			: `Turn on to mine up to ${limit} cards from each new video asbplayer opens, with audio and screenshots, as soon as it loads. Configure it in Mining → Auto Mode.`
	);
	// The start screen lines the toggle up with its buttons, so it gets no status line.
	const waiting = $derived(
		$autoMode && $autoCountdown === null && !$playerBusy && $fileResult !== null
	);
	let ledgerOpen = $state(false);
	const skipped = $derived(
		$autoMode &&
			$autoCountdown === null &&
			$autoSkipped !== null &&
			$autoSkipped === $fileResult?.batch_source.fingerprint
	);
</script>

{#if $autoAvailable || $autoMode}
	<div class="wrap">
		{#if waiting}
			<button
				class="status"
				aria-expanded={ledgerOpen}
				title="Videos auto mode processed this session"
				onclick={() => (ledgerOpen = !ledgerOpen)}
			>
				<AutoWaiting />
				{#if $autoLedger.length > 0}<span class="mined">✓ {$autoLedger.length}</span>{/if}
			</button>
		{/if}
		<button
			class="auto-toggle"
			class:on={$autoMode}
			role="switch"
			aria-checked={$autoMode}
			title={tip}
			onclick={() => void setAutoMode(!$autoMode)}
		>
			<span class="sparkle" aria-hidden="true">✦</span>
			<span class="label">Auto mode · {$autoMode ? 'On' : 'Off'}</span>
			<span class="track" aria-hidden="true"><span class="thumb"></span></span>
		</button>
		{#if ledgerOpen && waiting}
			<div class="countdown ledger-panel">
				<strong>Auto mode this session</strong>
				<AutoLedger />
			</div>
		{:else if $autoCountdown !== null}
			<div class="countdown" role="status">
				{#key $autoCountdown}<span class="secs">{$autoCountdown}</span>{/key}
				<span class="count-text">
					<strong>Mining this video</strong>
					<span>Click Auto mode to stop</span>
				</span>
			</div>
		{:else if skipped}
			<div class="countdown" role="status">
				<span class="count-text">
					<strong>Already mined this video</strong>
					<span class="actions">
						<button class="link" disabled={$playerBusy} onclick={() => void autoMine(true)}
							>Mine anyway</button
						>
						<button class="link" onclick={() => autoSkipped.set(null)}>Dismiss</button>
					</span>
				</span>
			</div>
		{/if}
	</div>
{/if}

<style>
	.auto-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.55rem;
		width: 12.5rem;
		height: 2.4rem;
		padding: 0 0.85rem;
		font-size: 0.9rem;
		color: var(--text);
		background: transparent;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
	}
	.auto-toggle:hover {
		border-color: var(--accent);
	}
	.auto-toggle.on {
		color: var(--accent);
		font-weight: 600;
		background: color-mix(in srgb, var(--accent) 18%, var(--bg-raised));
		border-color: var(--accent);
	}
	.sparkle {
		display: inline-block;
		color: var(--text-muted);
		font-size: 1rem;
	}
	.on .sparkle {
		color: var(--accent);
		animation: sparkle 0.6s ease-out;
	}
	.label {
		flex: 1;
		text-align: left;
		white-space: nowrap;
	}
	.track {
		position: relative;
		flex-shrink: 0;
		width: 1.9rem;
		height: 1.05rem;
		border-radius: var(--radius-pill);
		background: var(--bg-raised);
		border: 1px solid var(--border);
		transition: background 0.2s;
	}
	.thumb {
		position: absolute;
		top: 50%;
		left: 0.15rem;
		width: 0.7rem;
		height: 0.7rem;
		border-radius: 50%;
		background: var(--text-muted);
		transform: translateY(-50%);
		transition:
			left 0.2s,
			background 0.2s;
	}
	.on .track {
		background: var(--accent);
		border-color: var(--accent);
	}
	.on .thumb {
		left: calc(100% - 0.85rem);
		background: var(--bg-panel);
	}
	.wrap {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 0.3rem;
	}
	.status {
		display: inline-flex;
		align-items: center;
		gap: 0.6rem;
		padding: 0;
		background: none;
		border: none;
		cursor: pointer;
	}
	.mined {
		font-size: 0.8rem;
		color: var(--success);
	}
	.countdown.ledger-panel {
		display: grid;
		gap: 0.4rem;
		width: 20rem;
		font-size: 0.85rem;
	}
	.countdown {
		position: absolute;
		top: calc(100% + 0.5rem);
		right: 0;
		z-index: var(--z-toast);
		display: flex;
		align-items: center;
		gap: 0.7rem;
		width: 12.5rem;
		padding: 0.55rem 0.75rem;
		background: var(--bg-raised);
		border: 1px solid var(--accent);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
	}
	.secs {
		font-size: 1.8rem;
		font-weight: 700;
		line-height: 1;
		color: var(--accent);
		font-variant-numeric: tabular-nums;
		animation: tick 0.35s ease-out;
	}
	.count-text {
		display: grid;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.actions {
		display: flex;
		gap: 0.8rem;
		margin-top: 0.2rem;
	}
	.link {
		padding: 0;
		font-size: 0.8rem;
		color: var(--accent);
		background: none;
		border: none;
		cursor: pointer;
	}
	.link:hover:not(:disabled) {
		text-decoration: underline;
	}
	.link:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.count-text strong {
		color: var(--text);
		font-size: 0.85rem;
	}
	@keyframes tick {
		from {
			transform: scale(1.4);
			opacity: 0.4;
		}
	}
	@keyframes sparkle {
		0% {
			transform: scale(0.6) rotate(-45deg);
		}
		60% {
			transform: scale(1.35) rotate(15deg);
		}
		100% {
			transform: scale(1) rotate(0);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.on .sparkle,
		.secs {
			animation: none;
		}
		.track,
		.thumb {
			transition: none;
		}
	}
</style>
