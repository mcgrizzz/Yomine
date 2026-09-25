<script lang="ts">
	import Modal from './Modal.svelte';
	import BatchPauseDialog from './BatchPauseDialog.svelte';
	import BatchProgressDialog from './BatchProgressDialog.svelte';
	import BatchSummary from './BatchSummary.svelte';
	import {
		batchPause,
		batchPreview,
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
	import type { BatchRecord } from '$lib/ipc';

	const NARROW = 'min(30rem, calc(100vw - 2rem))';

	const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
	const count = (batch: BatchRecord, statuses: string[]) =>
		batch.items.filter((i) => statuses.includes(i.outcome.status)).length;
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
	{@const unchecked = count(previous, ['attempting'])}
	{@const notCreated = count(previous, ['unattempted', 'failed'])}
	<Modal
		title="Start a new batch?"
		width={NARROW}
		dismissible={false}
		onclose={() => confirmReplaceBatch(false)}
	>
		<div class="body">
			<p class="source" title={previous.source.title}>Last batch: {previous.source.title}</p>
			<ul>
				{#if previous.finished_at === null}<li>It stopped before finishing.</li>{/if}
				{#if notCreated > 0}<li>{plural(notCreated, 'card')} weren't created.</li>{/if}
				{#if unchecked > 0}
					<li>{plural(unchecked, 'card')} may or may not be in Anki; check before mining again.</li>
				{/if}
			</ul>
			<p>
				Yomine keeps only the most recent batch. After you start a new one, the last batch can't be
				retried or undone.
			</p>
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
	<BatchProgressDialog progress={$mineQueueState} batch={$lastBatch} preview={$batchPreview} />
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
	ul {
		margin: 0.4rem 0;
		padding-left: 1.2rem;
	}
	.source {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-muted);
		font-size: 0.85rem;
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
