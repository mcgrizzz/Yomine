<script lang="ts">
	import { fade } from 'svelte/transition';
	import Modal from './Modal.svelte';
	import { mediaParts } from '$lib/batch';
	import type { BatchRecord } from '$lib/ipc';
	import {
		cancelQueue,
		PREVIEW_SLOTS,
		type BatchPreview,
		type BatchProgress
	} from '$lib/stores/batches';
	import { miningMode } from '$lib/stores/settings';

	interface Props {
		progress: BatchProgress;
		batch: BatchRecord | null;
		previews: (BatchPreview | null)[];
	}

	let { progress, batch, previews }: Props = $props();

	const slots = $derived(Array.from({ length: PREVIEW_SLOTS }, (_, i) => previews[i] ?? null));
	const latest = $derived(
		slots.reduce<BatchPreview | null>((a, b) => (b && (!a || b.id > a.id) ? b : a), null)
	);
	const motionMs = window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 120;

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
		// Only what the "card N of M" heading doesn't already say.
		const parts = creating
			? [
					[outcomes.filter((o) => o.status === 'duplicate').length, 'already in Anki'],
					[outcomes.filter((o) => o.status === 'failed').length, 'not created']
				]
			: [[outcomes.filter((o) => mediaParts(o).includes('failed')).length, 'not recorded']];
		return parts.filter(([n]) => n).map(([n, label]) => `${n} ${label}`);
	});

	function stop() {
		stopping = true;
		cancelQueue();
	}
</script>

<Modal
	title="Mining batch"
	width="min(32rem, calc(100vw - 2rem))"
	dismissible={false}
	onclose={stop}
>
	<div class="body">
		<p class="step">
			{creating ? 'Creating' : 'Recording'} card {progress.position} of {progress.count}
		</p>
		<p class="muted explain">
			{#if !creating && $miningMode === 'local'}
				Cutting each line's audio and a screenshot from the video.
			{:else if !creating}
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

		{#if latest && !creating && $miningMode !== 'local'}
			<figure class="single">
				<img src={latest.src} alt="Screenshot recorded for {latest.lemma}" />
				<figcaption>
					<span class="eyebrow">Last recorded</span>
					<strong class="last-word" lang="ja">{latest.lemma}</strong>
					<span class="last-sentence" lang="ja" title={latest.sentence}>{latest.sentence}</span>
				</figcaption>
			</figure>
		{:else if latest && !creating}
			<div class="shots" role="list" aria-label="Recorded screenshots">
				{#each slots as shot, i (i)}
					<div class="shot" role="listitem" class:latest={shot?.id === latest.id}>
						{#if shot}
							{#key shot.id}
								<img
									src={shot.src}
									alt="Screenshot recorded for {shot.lemma}"
									title={shot.lemma}
									transition:fade={{ duration: motionMs }}
								/>
							{/key}
						{/if}
					</div>
				{/each}
			</div>
		{/if}

		{#if counts.length > 0}
			<p class="muted counts">{counts.join(' · ')}</p>
		{/if}
	</div>
	{#snippet footer()}
		<footer>
			<span class="muted">
				{stopping
					? 'Stopping after the current card…'
					: 'Stopping finishes the current card first.'}
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
		.fill,
		.shot {
			transition: none;
		}
	}
	.shots {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 0.3rem;
		margin-top: 0.6rem;
	}
	.shot {
		position: relative;
		aspect-ratio: 16 / 9;
		overflow: hidden;
		border-radius: var(--radius-sm);
		background: var(--bg-raised);
		outline: 2px solid transparent;
		outline-offset: -2px;
		transition: outline-color 0.12s;
	}
	.shot.latest {
		outline-color: var(--accent);
	}
	.shot img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.single {
		display: flex;
		align-items: center;
		gap: 0.85rem;
		margin: 0.6rem 0 0;
		padding: 0.6rem;
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.single img {
		flex-shrink: 0;
		width: 7.5rem;
		aspect-ratio: 16 / 9;
		object-fit: cover;
		border-radius: var(--radius-sm);
		background: var(--bg-deep);
	}
	.single figcaption {
		display: grid;
		gap: 0.15rem;
		min-width: 0;
	}
	.eyebrow {
		color: var(--text-muted);
		font-size: 0.7rem;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	.last-word {
		font-size: 1rem;
	}
	.last-sentence {
		display: -webkit-box;
		overflow: hidden;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.4;
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
