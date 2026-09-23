<script lang="ts">
	import Modal from './Modal.svelte';
	import BatchPauseDialog from './BatchPauseDialog.svelte';
	import BatchProgressDialog from './BatchProgressDialog.svelte';
	import BatchSummary from './BatchSummary.svelte';
	import {
		batchPause,
		batchReplace,
		batchTarget,
		confirmReplaceBatch,
		lastBatch,
		mineQueueState,
		pauseChoices,
		resumeBatch,
		selectBatchTarget
	} from '$lib/stores/batches';
	import { batchSummaryOpen } from '$lib/stores/modals';

	const NARROW = 'min(30rem, calc(100vw - 2rem))';
</script>

{#if $batchTarget}
	{@const choice = $batchTarget}
	<Modal
		title="Choose the recording tab"
		width={NARROW}
		dismissible={false}
		onclose={() => selectBatchTarget(null)}
	>
		<div class="body">
			<p>
				These subtitles were opened from a file. Choose the asbplayer video that should supply the
				audio and screenshots.
			</p>
			{#each choice.media as target (target.id)}
				{@const usable = target.active && target.loaded_subtitles.length > 0}
				<button
					class="target"
					disabled={!usable}
					onclick={() => selectBatchTarget({ target: target.id, record: true })}
				>
					{target.title ?? target.id}
					{#if !target.active}
						<span class="muted">— activate this tab first</span>
					{:else if target.loaded_subtitles.length === 0}
						<span class="muted">— load subtitles first</span>
					{/if}
				</button>
			{:else}
				<p class="muted">No video is available in asbplayer.</p>
			{/each}
		</div>
		{#snippet footer()}
			<footer>
				{#if !choice.mediaRun}
					<button onclick={() => selectBatchTarget({ target: null, record: false })}>
						Mine without recording
					</button>
				{/if}
				<button class="end" onclick={() => selectBatchTarget(null)}>Cancel</button>
			</footer>
		{/snippet}
	</Modal>
{:else if $batchReplace}
	{@const previous = $batchReplace}
	<Modal
		title="Replace the last batch?"
		width={NARROW}
		dismissible={false}
		onclose={() => confirmReplaceBatch(false)}
	>
		<div class="body">
			<p>
				The last batch from <strong>{previous.source.title}</strong> was interrupted or has cards
				that need review in Anki.
			</p>
			<p>Starting a new batch replaces its record, so it can no longer be retried or undone.</p>
		</div>
		{#snippet footer()}
			<footer>
				<button onclick={() => confirmReplaceBatch(false)}>Review last batch</button>
				<button class="primary end" onclick={() => confirmReplaceBatch(true)}>
					Start new batch
				</button>
			</footer>
		{/snippet}
	</Modal>
{:else if $batchPause}
	<BatchPauseDialog pause={$batchPause} choices={pauseChoices($batchPause)} onchoose={resumeBatch} />
{:else if $mineQueueState}
	<BatchProgressDialog progress={$mineQueueState} batch={$lastBatch} />
{:else if $batchSummaryOpen && $lastBatch}
	<BatchSummary batch={$lastBatch} onclose={() => batchSummaryOpen.set(false)} />
{/if}

<style>
	.body {
		padding: 0 1rem 0.5rem;
	}
	p {
		margin: 0.6rem 0;
	}
	footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem 1rem 0;
		border-top: 1px solid var(--border);
	}
	.end {
		margin-left: auto;
	}
	.primary {
		border-color: var(--accent);
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.target {
		display: block;
		width: 100%;
		margin: 0.5rem 0;
		text-align: left;
	}
</style>
