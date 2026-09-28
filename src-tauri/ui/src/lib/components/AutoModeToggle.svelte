<script lang="ts">
	import AutoLedger from './AutoLedger.svelte';
	import AutoWaiting from './AutoWaiting.svelte';
	import {
		autoAvailable,
		autoCountdown,
		autoLedger,
		autoMode,
		autoReview,
		autoSkipped,
		fileResult,
		miningMode,
		playerBusy,
		processAnyway,
		reviewDialogOpen,
		selectedTerms,
		selectPicks,
		setAutoMode,
		settings,
		showNotice,
		type AutoMode
	} from '$lib/stores';

	const LABELS: Record<AutoMode, string> = { off: 'Off', review: 'Review', mine: 'Mine' };

	const amount = $derived.by(() => {
		const prefs = $settings?.auto_mine;
		if (prefs?.stop !== 'min_score') return `up to ${prefs?.limit ?? 10} cards`;
		return `every term scoring at least ${prefs.min_score}`;
	});
	const newVideo = $derived(
		$miningMode === 'local' ? 'a video opens' : 'asbplayer opens a new video'
	);
	const descriptions = $derived<Record<AutoMode, string>>({
		off: `Nothing happens when ${newVideo}.`,
		review: `When ${newVideo}, select ${amount} and show them for you to check before mining.`,
		mine: `When ${newVideo}, mine ${amount} after a short countdown, without asking.`
	});

	// The start screen lines the toggle up with its buttons, so it gets no status line.
	const waiting = $derived(
		$autoMode !== 'off' &&
			$autoCountdown === null &&
			$autoReview === null &&
			!$playerBusy &&
			$fileResult !== null
	);
	let ledgerOpen = $state(false);
	let menuOpen = $state(false);
	const skipped = $derived(
		$autoMode !== 'off' &&
			$autoCountdown === null &&
			$autoSkipped !== null &&
			$autoSkipped === $fileResult?.batch_source.fingerprint
	);
	const waitingPicks = $derived(
		$autoReview?.keys.filter((k) => $selectedTerms.has(k)).length ?? 0
	);

	function choose(mode: AutoMode) {
		menuOpen = false;
		void setAutoMode(mode);
	}

	async function pickNow() {
		const result = await selectPicks();
		if (!result) return;
		const n = result.picks.length;
		showNotice(
			n === 0 ? 'Auto mode found nothing to pick in this file' : `Selected ${n} pick${n === 1 ? '' : 's'}`
		);
	}
</script>

<svelte:window
	onclick={() => (menuOpen = false)}
	onkeydown={(e) => e.key === 'Escape' && (menuOpen = false)}
/>

<div class="row">
	{#if $autoReview !== null && !$reviewDialogOpen}
		<button
			class="select-picks waiting-review"
			title="Auto mode's picks for this video are selected and waiting for you to check them"
			onclick={() => reviewDialogOpen.set(true)}
		>
			<span class="sparkle" aria-hidden="true">✦</span> Review {waitingPicks} pick{waitingPicks === 1
				? ''
				: 's'}
		</button>
	{:else if $fileResult !== null}
		<button
			class="select-picks"
			disabled={$playerBusy}
			title="Select what auto mode would pick from this file. Mining stays manual."
			onclick={() => void pickNow()}
		>
			<span class="sparkle" aria-hidden="true">✦</span> Select picks
		</button>
	{/if}
	{#if $autoAvailable || $autoMode !== 'off'}
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
				data-mode={$autoMode}
				aria-haspopup="menu"
				aria-expanded={menuOpen}
				title="Choose what auto mode does when {newVideo}. Configure its picks in Mining → Auto Mode."
				onclick={(e) => {
					e.stopPropagation();
					menuOpen = !menuOpen;
				}}
			>
				<span class="sparkle" aria-hidden="true">✦</span>
				<span class="label">Auto mode · {LABELS[$autoMode]}</span>
				<span class="track" aria-hidden="true"><span class="thumb"></span></span>
			</button>
			{#if menuOpen}
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions --
				     the handler only fences the window's close-on-outside-click listener. -->
				<div class="card menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
					{#each ['off', 'review', 'mine'] as const as mode (mode)}
						<button
							class="option"
							data-mode={mode}
							role="menuitemradio"
							aria-checked={$autoMode === mode}
							onclick={() => choose(mode)}
						>
							<span class="track" aria-hidden="true"><span class="thumb"></span></span>
							<span class="option-name">{LABELS[mode]}</span>
							<span class="option-desc">{descriptions[mode]}</span>
						</button>
					{/each}
					<p class="menu-foot">Mine turns back into Review when Yomine restarts.</p>
				</div>
			{:else if ledgerOpen && waiting}
				<div class="card ledger-panel">
					<strong>Auto mode this session</strong>
					<AutoLedger />
				</div>
			{:else if $autoCountdown !== null}
				<div class="card countdown" role="status">
					{#key $autoCountdown}<span class="secs">{$autoCountdown}</span>{/key}
					<span class="count-text">
						<strong>Mining this video</strong>
						<span>Pick Off or Review to stop</span>
					</span>
				</div>
			{:else if skipped}
				<div class="card" role="status">
					<span class="count-text">
						<strong>Already mined this video</strong>
						<span class="actions">
							<button class="link" disabled={$playerBusy} onclick={() => void processAnyway()}
								>{$autoMode === 'review' ? 'Review anyway' : 'Mine anyway'}</button
							>
							<button class="link" onclick={() => autoSkipped.set(null)}>Dismiss</button>
						</span>
					</span>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.row {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		gap: 0.5rem;
	}
	.select-picks {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		height: 2.4rem;
		padding: 0 0.85rem;
		font-size: 0.9rem;
		color: var(--text);
		background: transparent;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
		white-space: nowrap;
	}
	.select-picks:hover:not(:disabled) {
		border-color: var(--accent);
	}
	.select-picks:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.select-picks .sparkle {
		color: var(--accent);
	}
	.select-picks.waiting-review {
		color: var(--accent);
		font-weight: 600;
		background: color-mix(in srgb, var(--accent) 12%, var(--bg-raised));
		border-color: var(--accent);
	}
	.auto-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.55rem;
		/* Fits "Review", so the width never changes. */
		width: 15.5rem;
		height: 2.4rem;
		padding: 0 0.85rem;
		font-size: 0.9rem;
		color: var(--text);
		background: transparent;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		cursor: pointer;
		transition:
			background 0.2s,
			border-color 0.2s;
	}
	.auto-toggle:hover {
		border-color: var(--accent);
	}
	.auto-toggle[data-mode='review'] {
		background: color-mix(in srgb, var(--accent) 8%, var(--bg-raised));
		border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
	}
	.auto-toggle[data-mode='mine'] {
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
	[data-mode='review'] .sparkle,
	[data-mode='mine'] .sparkle {
		color: var(--accent);
	}
	.auto-toggle[data-mode='mine'] .sparkle {
		animation: sparkle 0.6s ease-out;
	}
	.label {
		flex: 1;
		min-width: 0;
		text-align: left;
		white-space: nowrap;
	}
	.track {
		position: relative;
		flex-shrink: 0;
		width: 2.5rem;
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
	[data-mode='review'] .track {
		background: color-mix(in srgb, var(--accent) 35%, var(--bg-raised));
		border-color: color-mix(in srgb, var(--accent) 60%, var(--border));
	}
	[data-mode='review'] .thumb {
		left: calc(50% - 0.35rem);
		background: var(--text);
	}
	[data-mode='mine'] .track {
		background: var(--accent);
		border-color: var(--accent);
	}
	[data-mode='mine'] .thumb {
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
	.card {
		position: absolute;
		top: calc(100% + 0.5rem);
		right: 0;
		z-index: var(--z-toast);
		display: flex;
		align-items: center;
		gap: 0.7rem;
		width: 15.5rem;
		padding: 0.55rem 0.75rem;
		background: var(--bg-raised);
		border: 1px solid var(--accent);
		border-radius: var(--radius);
		box-shadow: var(--shadow-overlay);
	}
	.card.ledger-panel {
		display: grid;
		gap: 0.4rem;
		width: 20rem;
		font-size: 0.85rem;
	}
	.menu {
		display: grid;
		gap: 0.1rem;
		width: 19.5rem;
		padding: 0.3rem;
	}
	.option {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.1rem 0.65rem;
		align-items: center;
		padding: 0.5rem 0.55rem;
		text-align: left;
		color: var(--text);
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
	}
	.option:hover {
		background: var(--bg-hover);
	}
	.option[aria-checked='true'] {
		background: color-mix(in srgb, var(--accent) 12%, transparent);
	}
	.option .track {
		grid-row: span 2;
		width: 1.9rem;
		height: 0.9rem;
	}
	.option .thumb {
		width: 0.55rem;
		height: 0.55rem;
	}
	.option[data-mode='review'] .thumb {
		left: calc(50% - 0.275rem);
	}
	.option[data-mode='mine'] .thumb {
		left: calc(100% - 0.7rem);
	}
	.option-name {
		font-size: 0.88rem;
		font-weight: 600;
	}
	.option[aria-checked='true'] .option-name::after {
		content: '  ✓';
		color: var(--accent);
	}
	.option-desc {
		font-size: 0.78rem;
		line-height: 1.4;
		color: var(--text-muted);
	}
	.menu-foot {
		margin: 0.2rem 0 0;
		padding: 0.4rem 0.55rem 0.3rem;
		font-size: 0.74rem;
		color: var(--text-muted);
		border-top: 1px solid var(--border);
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
		.auto-toggle,
		.auto-toggle .sparkle,
		.secs {
			animation: none;
			transition: none;
		}
		.track,
		.thumb {
			transition: none;
		}
	}
</style>
