<script lang="ts">
	import Modal from './Modal.svelte';
	import type { BatchRecord } from '$lib/ipc';
	import { cancelQueue, type BatchProgress } from '$lib/stores/batches';

	let { progress, batch }: { progress: BatchProgress; batch: BatchRecord | null } = $props();

	let stopping = $state(false);
	let now = $state(Date.now());

	$effect(() => {
		const tick = setInterval(() => (now = Date.now()), 500);
		return () => clearInterval(tick);
	});

	const creating = $derived(progress.phase === 'create');
	const progressed = $derived(
		progress.doneSecs + Math.min((now - progress.estimatedAt) / 1000, progress.currentSecs)
	);
	const percent = $derived(
		progress.totalSecs > 0 ? Math.round((progressed / progress.totalSecs) * 100) : 0
	);
	const timeLeft = $derived.by(() => {
		const secs = Math.round(progress.totalSecs - progressed);
		if (secs < 3) return 'Almost done';
		return `About ${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')} left`;
	});
	const counts = $derived.by(() => {
		const outcomes = batch?.items.map((i) => i.outcome) ?? [];
		const created = outcomes.filter((o) => o.status === 'created');
		const parts = creating
			? [
					[created.length, 'created'],
					[outcomes.filter((o) => o.status === 'duplicate').length, 'already in Anki'],
					[outcomes.filter((o) => o.status === 'failed').length, 'not created']
				]
			: [
					[created.filter((o) => o.media === 'complete').length, 'recorded'],
					[created.filter((o) => o.media === 'failed').length, 'not recorded']
				];
		return parts.filter(([n]) => n).map(([n, label]) => `${n} ${label}`);
	});

	function stop() {
		stopping = true;
		cancelQueue();
	}
</script>

<Modal title="Mining batch" width="min(32rem, calc(100vw - 2rem))" dismissible={false} onclose={stop}>
	<div class="body">
		<p class="step">
			{creating ? 'Creating' : 'Recording'} card {progress.position} of {progress.count}
		</p>
		<p class="muted explain">
			{#if !creating}
				asbplayer plays each line in the video tab to record it. Keep that tab open.
			{:else if progress.recordsNext}
				Adding the cards to Anki first. Audio and screenshots are recorded once every card exists.
			{:else}
				Adding the cards to Anki.
			{/if}
		</p>

		<div class="progress">
			<div
				class="track"
				role="progressbar"
				aria-valuemin={0}
				aria-valuemax={100}
				aria-valuenow={percent}
			>
				<div class="fill" style:width="{percent}%"></div>
			</div>
			<span class="fraction">{timeLeft}</span>
		</div>

		<div class="current">
			<strong class="word" lang="ja">{progress.current}</strong>
			<span class="sentence" lang="ja" title={progress.sentence}>{progress.sentence}</span>
			<span class="status">
				<span class="pulse" aria-hidden="true"></span>
				{progress.message ?? 'Starting…'}
			</span>
		</div>

		{#if counts.length > 0}
			<p class="muted counts">{counts.join(' · ')}</p>
		{/if}
	</div>
	{#snippet footer()}
		<footer>
			<span class="muted">
				{stopping ? 'Stopping after the current card…' : 'Stopping finishes the current card first.'}
			</span>
			<button disabled={stopping} onclick={stop}>Stop</button>
		</footer>
	{/snippet}
</Modal>

<style>
	.body {
		padding: 0.25rem 1rem 0.75rem;
	}
	p {
		margin: 0;
	}
	.step {
		font-weight: 600;
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
		font-weight: normal;
	}
	.explain {
		margin-top: 0.2rem;
	}
	.progress {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin: 1rem 0;
	}
	.track {
		flex: 1;
		height: 0.5rem;
		overflow: hidden;
		border-radius: var(--radius-pill);
		background: var(--bg-raised);
	}
	.fill {
		height: 100%;
		border-radius: inherit;
		background: var(--accent);
		transition: width 0.5s linear;
	}
	.fraction {
		color: var(--text-muted);
		font-size: 0.8rem;
		font-variant-numeric: tabular-nums;
	}
	.current {
		display: grid;
		gap: 0.2rem;
		padding: 0.75rem 0.85rem;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--bg-raised);
	}
	.word {
		font-size: 1.2rem;
	}
	.sentence {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		margin-top: 0.3rem;
		font-size: 0.85rem;
	}
	.pulse {
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 50%;
		background: var(--accent);
		animation: pulse 1.2s ease-in-out infinite;
	}
	@keyframes pulse {
		50% {
			opacity: 0.3;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.pulse {
			animation: none;
		}
		.fill {
			transition: none;
		}
	}
	.counts {
		margin-top: 0.75rem;
	}
	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.75rem 1rem 0;
		border-top: 1px solid var(--border);
	}
</style>
