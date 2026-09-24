<script lang="ts">
	import Modal from './Modal.svelte';
	import RecordingSteps from './RecordingSteps.svelte';
	import AutoLedger from './AutoLedger.svelte';
	import AutoWaiting from './AutoWaiting.svelte';
	import { openNotesInAnki, type BatchItem, type BatchRecord } from '$lib/ipc';
	import { lacksMedia, retryIndices, sameSource } from '$lib/batch';
	import {
		batchSaveError,
		restoreBatchSelection,
		retryBatch,
		undoLastBatch
	} from '$lib/stores/batches';
	import { autoMode } from '$lib/stores/auto';
	import { fileResult } from '$lib/stores/file';
	import { openInAnki, playerBusy } from '$lib/stores/mining';
	import { lastError } from '$lib/stores/ui';

	let { batch, onclose }: { batch: BatchRecord; onclose: () => void } = $props();

	type Row = { item: BatchItem; index: number };
	type Group = 'review' | 'notCreated' | 'noMedia' | 'created' | 'duplicate' | 'deleted';
	type Tag = { text: string; detail: string | null };

	const SEGMENTS: { group: Group; label: string; tone: string }[] = [
		{ group: 'created', label: 'created', tone: 'ok' },
		{ group: 'noMedia', label: 'without media', tone: 'warn' },
		{ group: 'duplicate', label: 'already in Anki', tone: 'muted' },
		{ group: 'review', label: 'need review', tone: 'warn' },
		{ group: 'notCreated', label: 'not created', tone: 'bad' },
		{ group: 'deleted', label: 'undone', tone: 'muted' }
	];

	let confirmUndo = $state(false);
	let expanded = $state(new Set<number>());

	const groups = $derived.by(() => {
		const rows: Record<Group, Row[]> = {
			review: [],
			notCreated: [],
			noMedia: [],
			created: [],
			duplicate: [],
			deleted: []
		};
		batch.items.forEach((item, index) => rows[groupOf(item)].push({ item, index }));
		return rows;
	});
	const createdIds = $derived(
		batch.items.flatMap((i) => (i.outcome.status === 'created' ? [i.outcome.note_id] : []))
	);
	const retryCount = $derived(retryIndices(batch).length);
	const mediaCount = $derived(retryIndices(batch, true).length);
	const sourceMatches = $derived(sameSource(batch, $fileResult));
	const blocked = $derived($playerBusy || !!$batchSaveError || !sourceMatches);
	const unattempted = $derived(
		groups.notCreated.filter((r) => r.item.outcome.status === 'unattempted').length
	);
	const status = $derived(
		batch.finished_at === null ? 'Interrupted' : unattempted > 0 ? 'Stopped early' : 'Complete'
	);
	const unrecorded = $derived(
		groups.noMedia.some(({ item: { outcome: o } }) => {
			return o.status === 'created' && o.error?.kind === 'media_unverified';
		})
	);
	const finished = $derived([...groups.created, ...groups.duplicate, ...groups.deleted]);
	const needsAttention = $derived(
		groups.review.length + groups.notCreated.length + groups.noMedia.length > 0
	);

	function groupOf(item: BatchItem): Group {
		const o = item.outcome;
		if (o.status === 'attempting') return 'review';
		if (o.status === 'failed' || o.status === 'unattempted') return 'notCreated';
		if (o.status === 'duplicate') return 'duplicate';
		if (o.status === 'deleted') return 'deleted';
		return lacksMedia(o) ? 'noMedia' : 'created';
	}

	function tagOf(item: BatchItem): Tag | null {
		const o = item.outcome;
		if (o.status === 'failed') return { text: o.error.stage, detail: o.error.message };
		if (o.status === 'duplicate') return { text: 'Already in Anki', detail: null };
		if (o.status === 'deleted') return { text: 'Undone', detail: null };
		if (o.status !== 'created' || !lacksMedia(o)) return null;
		if (o.media === 'skipped') return { text: 'Recording skipped', detail: null };
		if (!o.error) return { text: 'Not recorded yet', detail: null };
		const text = o.error.kind === 'media_unverified' ? 'Not recorded' : o.error.stage;
		return { text, detail: o.error.message };
	}

	function toggle(index: number) {
		const next = new Set(expanded);
		if (!next.delete(index)) next.add(index);
		expanded = next;
	}

	const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

	async function openCreated() {
		try {
			await openNotesInAnki(createdIds);
		} catch (e) {
			lastError.set({ title: 'Open in Anki', message: String(e), detail: null });
		}
	}
</script>

{#snippet rowList(rows: Row[])}
	<ul>
		{#each rows as { item, index } (`${batch.id}:${index}`)}
			{@const outcome = item.outcome}
			{@const tag = tagOf(item)}
			<li>
				<div class="row">
					{#if outcome.status === 'created'}
						<button
							class="word link"
							lang="ja"
							title="Open in Anki"
							onclick={() => openInAnki(outcome.note_id)}>{item.lemma}</button
						>
					{:else}
						<span class="word" lang="ja">{item.lemma}</span>
					{/if}
					<span class="sentence" lang="ja" title={item.sentence}>{item.sentence}</span>
					{#if item.adhoc}<span class="tag">Ad-hoc</span>{/if}
					{#if tag?.detail}
						<button
							class="tag expandable"
							aria-expanded={expanded.has(index)}
							onclick={() => toggle(index)}>{tag.text} {expanded.has(index) ? '▴' : '▾'}</button
						>
					{:else if tag}
						<span class="tag">{tag.text}</span>
					{/if}
				</div>
				{#if tag?.detail && expanded.has(index)}<p class="detail">{tag.detail}</p>{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<Modal title="Batch summary" width="min(44rem, calc(100vw - 2rem))" {onclose}>
	<div class="body">
		<header class="heading">
			<h2 title={batch.source.title}>{batch.source.title}</h2>
			<p class="meta">
				{new Date(batch.started_at).toLocaleString()} ·
				<span class:attention={status !== 'Complete'}>{status}</span>
				· {plural(batch.items.length, 'card')}
			</p>
		</header>

		<div class="bar" aria-hidden="true">
			{#each SEGMENTS as s (s.group)}
				{#if groups[s.group].length > 0}
					<span class="segment {s.tone}" style:flex-grow={groups[s.group].length}></span>
				{/if}
			{/each}
		</div>
		<ul class="legend">
			{#each SEGMENTS as s (s.group)}
				{#if groups[s.group].length > 0}
					<li><span class="dot {s.tone}"></span><strong>{groups[s.group].length}</strong> {s.label}</li>
				{/if}
			{/each}
		</ul>

		{#if batch.auto && $autoMode}
			<details class="auto">
				<summary><AutoWaiting /></summary>
				<AutoLedger />
			</details>
		{/if}

		{#if $batchSaveError}
			<p class="notice alert">
				{$batchSaveError} Retry and undo are unavailable until the saved record can be verified.
			</p>
		{/if}
		{#if !sourceMatches}
			<p class="notice">Load the original source to retry cards or restore the selection.</p>
		{/if}

		<div class="scroll">
			{#if groups.review.length > 0}
				<section class="panel warn">
					<div class="panel-head">
						<div>
							<h3>{plural(groups.review.length, 'card')} to check in Anki</h3>
							<p>Anki didn't confirm these. Check whether each exists before mining it again.</p>
						</div>
					</div>
					{@render rowList(groups.review)}
				</section>
			{/if}

			{#if groups.notCreated.length > 0}
				<section class="panel bad">
					<div class="panel-head">
						<div>
							<h3>{plural(groups.notCreated.length, 'card')} not created</h3>
							<p>
								{unattempted === groups.notCreated.length
									? 'The batch stopped before these were mined.'
									: 'Open a tag to see why a card failed.'}
							</p>
						</div>
						<button class="primary" disabled={blocked || retryCount === 0} onclick={() => retryBatch()}>
							Mine {plural(retryCount, 'card')}
						</button>
					</div>
					{@render rowList(groups.notCreated)}
				</section>
			{/if}

			{#if groups.noMedia.length > 0}
				<section class="panel warn">
					<div class="panel-head">
						<div>
							<h3>{plural(groups.noMedia.length, 'card')} without audio or screenshot</h3>
							<p>These cards are in Anki. Retry media adds the recording to the same cards.</p>
						</div>
						<button class="primary" disabled={blocked || mediaCount === 0} onclick={() => retryBatch(true)}>
							Retry media
						</button>
					</div>
					{#if unrecorded}
						<div class="steps"><RecordingSteps retryLabel="Retry media" /></div>
					{/if}
					{@render rowList(groups.noMedia)}
				</section>
			{/if}

			{#if finished.length > 0}
				<details class="finished" open={!needsAttention}>
					<summary>
						Finished
						<span class="count">
							{[
								groups.created.length && `${groups.created.length} created`,
								groups.duplicate.length && `${groups.duplicate.length} already in Anki`,
								groups.deleted.length && `${groups.deleted.length} undone`
							]
								.filter(Boolean)
								.join(' · ')}
						</span>
					</summary>
					{@render rowList(finished)}
				</details>
			{/if}
		</div>
	</div>

	{#snippet footer()}
		<footer>
			{#if confirmUndo}
				<span class="confirm">
					Delete {plural(createdIds.length, 'note')} from Anki? Media files are kept.
				</span>
				<button disabled={$playerBusy} onclick={() => (confirmUndo = false)}>Cancel</button>
				<button
					class="danger-solid"
					disabled={$playerBusy}
					onclick={async () => {
						await undoLastBatch();
						confirmUndo = false;
					}}>Delete notes</button
				>
			{:else}
				<button
					class="undo"
					disabled={$playerBusy || !!$batchSaveError || createdIds.length === 0}
					onclick={() => (confirmUndo = true)}>Undo batch…</button
				>
				<span class="spacer"></span>
				<button disabled={$playerBusy || !sourceMatches} onclick={restoreBatchSelection}>
					Restore selection
				</button>
				<button disabled={createdIds.length === 0} onclick={openCreated}>
					Open in Anki ({createdIds.length})
				</button>
				<button class="primary" onclick={onclose}>Done</button>
			{/if}
		</footer>
	{/snippet}
</Modal>

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 0.25rem 1rem 0.5rem;
	}
	.auto {
		padding: 0.4rem 0.6rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.auto summary {
		cursor: pointer;
	}
	.auto[open] summary {
		margin-bottom: 0.4rem;
	}
	.heading h2 {
		margin: 0;
		font-size: 1rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.meta {
		margin: 0.2rem 0 0;
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.attention {
		color: var(--warning);
	}
	.bar {
		display: flex;
		gap: 2px;
		height: 0.5rem;
		overflow: hidden;
		border-radius: var(--radius-pill);
		background: var(--bg-raised);
	}
	.segment {
		flex-basis: 0;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem 1rem;
		margin: -0.25rem 0 0;
		padding: 0;
		list-style: none;
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	.legend strong {
		color: var(--text);
	}
	.dot {
		display: inline-block;
		width: 0.55rem;
		height: 0.55rem;
		margin-right: 0.35rem;
		border-radius: 50%;
	}
	.segment.ok,
	.dot.ok {
		background: var(--success);
	}
	.segment.warn,
	.dot.warn {
		background: var(--warning);
	}
	.segment.bad,
	.dot.bad {
		background: var(--danger);
	}
	.segment.muted,
	.dot.muted {
		background: var(--text-muted);
	}
	.notice {
		margin: 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}
	.notice.alert {
		color: var(--warning);
	}
	.scroll {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		max-height: 52vh;
		overflow: auto;
	}
	.panel {
		border: 1px solid var(--border);
		border-left: 3px solid var(--border-control);
		border-radius: var(--radius);
		background: var(--bg-raised);
	}
	.panel.warn {
		border-left-color: var(--warning);
	}
	.panel.bad {
		border-left-color: var(--danger);
	}
	.panel-head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.7rem 0.85rem 0.5rem;
	}
	.panel-head button {
		flex-shrink: 0;
	}
	h3 {
		margin: 0;
		font-size: 0.95rem;
	}
	.panel-head p {
		margin: 0.2rem 0 0;
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	.steps {
		margin: 0 0.85rem 0.6rem;
		padding: 0.6rem 0.75rem;
		border-radius: var(--radius-sm);
		background: var(--bg-panel);
		font-size: 0.85rem;
	}
	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	li {
		padding: 0.4rem 0.85rem;
		border-top: 1px solid var(--border);
	}
	.row {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		min-width: 0;
	}
	.word {
		flex-shrink: 0;
		font-weight: 600;
	}
	.link {
		padding: 0;
		border: 0;
		background: none;
		color: var(--link);
	}
	.link:hover:not(:disabled) {
		background: none;
		text-decoration: underline;
	}
	.sentence {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.tag {
		flex-shrink: 0;
		padding: 0.05rem 0.5rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-pill);
		background: none;
		color: var(--text-muted);
		font-size: 0.75rem;
	}
	.tag.expandable:hover:not(:disabled) {
		border-color: var(--accent);
		color: var(--text);
		background: none;
	}
	.detail {
		margin: 0.4rem 0 0.1rem;
		padding: 0.5rem 0.65rem;
		border-radius: var(--radius-sm);
		background: var(--bg-panel);
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.45;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.finished {
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.finished summary {
		padding: 0.6rem 0.85rem;
		font-weight: 600;
		font-size: 0.95rem;
		cursor: pointer;
	}
	.finished .count {
		margin-left: 0.4rem;
		color: var(--text-muted);
		font-size: 0.8rem;
		font-weight: normal;
	}
	footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem 1rem 0;
		border-top: 1px solid var(--border);
	}
	.spacer {
		flex: 1;
	}
	.primary {
		border-color: var(--accent);
		background: color-mix(in srgb, var(--accent) 14%, var(--bg-raised));
	}
	.undo {
		border-color: transparent;
		background: none;
		color: var(--danger);
	}
	.undo:hover:not(:disabled) {
		border-color: var(--danger);
		background: none;
	}
	.confirm {
		flex: 1;
		font-size: 0.85rem;
	}
	.danger-solid {
		border-color: var(--danger);
		background: color-mix(in srgb, var(--danger) 18%, var(--bg-raised));
	}
</style>
