<script lang="ts">
	import Modal from './Modal.svelte';
	import BatchSummary from './BatchSummary.svelte';
	import { getMediaMatches, listBatches, type BatchRecord, type Media } from '$lib/ipc';
	import { mediaTitle } from '$lib/media';
	import { batchStatus, undone, unfinished } from '$lib/batch';
	import { lastBatch } from '$lib/stores/batches';
	import { batchHistoryOpen } from '$lib/stores/modals';

	type Group = { key: string; show: string; batches: BatchRecord[] };

	// The episode suffix the file-name parser's display_title adds.
	const EPISODE = / - (S\d+E\d+|Episode [\d.]+)$/;

	let batches = $state<BatchRecord[] | null>(null);
	/** Matched sources' shows and episodes, by fingerprint. */
	let media = $state<Record<string, Media>>({});
	let error = $state<string | null>(null);
	let selected = $state<string | null>(null);
	/** Groups opened or closed against the default, by their newest batch's id. */
	let toggled = $state(new Set<string>());

	listBatches().then(
		(list) => {
			batches = list;
			void getMediaMatches([...new Set(list.map((b) => b.source.fingerprint))]).then(
				(found) => (media = found)
			);
		},
		(e) => (error = String(e))
	);

	// The newest batch changes when it's undone or retried.
	const rows = $derived((batches ?? []).map((b) => (b.id === $lastBatch?.id ? $lastBatch : b)));
	const shown = $derived(rows.find((b) => b.id === selected));
	const groups = $derived.by(() => {
		const out: Group[] = [];
		const named = (batch: BatchRecord) => batch.source.title.replace(EPISODE, '');
		// An unmatched batch joins the show a matched batch of the same name is matched to.
		const works = new Map<string, Media['work']>();
		for (const batch of rows) {
			const matched = media[batch.source.fingerprint];
			if (matched) works.set(named(batch), matched.work);
		}
		for (const batch of rows) {
			const work = media[batch.source.fingerprint]?.work ?? works.get(named(batch));
			const show = work ? work.original_title || work.title : named(batch);
			const key = work ? `${work.kind}:${work.tmdb_id}` : show;
			const last = out.at(-1);
			if (last?.key === key) last.batches.push(batch);
			else out.push({ key, show, batches: [batch] });
		}
		return out;
	});

	const close = () => batchHistoryOpen.set(false);
	const plural = (n: number, word: string, many = `${word}s`) => `${n} ${n === 1 ? word : many}`;
	const count = (batch: BatchRecord, statuses: string[]) =>
		batch.items.filter((i) => statuses.includes(i.outcome.status)).length;
	function title(batch: BatchRecord) {
		const matched = media[batch.source.fingerprint];
		return matched ? mediaTitle(matched) : batch.source.title;
	}
	function episode(batch: BatchRecord) {
		const matched = media[batch.source.fingerprint];
		if (matched?.season != null && matched.episode !== null)
			return `S${matched.season}E${matched.episode}`;
		return batch.source.title.match(EPISODE)?.[1] ?? batch.source.title;
	}
	const sameDay = (a: number, b: number) =>
		new Date(a).toDateString() === new Date(b).toDateString();

	function isOpen(group: Group) {
		const latest = group.batches.some((b) => b.id === $lastBatch?.id);
		return latest !== toggled.has(group.batches[0].id);
	}

	function toggle(group: Group) {
		const next = new Set(toggled);
		if (!next.delete(group.batches[0].id)) next.add(group.batches[0].id);
		toggled = next;
	}
</script>

{#snippet meta(batch: BatchRecord)}
	{@const status = batchStatus(batch)}
	{@const notCreated = count(batch, ['failed', 'unattempted'])}
	{@const undoneCards = count(batch, ['deleted'])}
	<span class="meta">
		<span class:attention={unfinished(status)} class:undone={undone(status)}>{status}</span>
		· {plural(batch.items.length, 'card')}
		{#if notCreated}· {notCreated} not created{/if}
		{#if undoneCards && status !== 'Undone'}· {undoneCards} undone{/if}
		{#if batch.auto}· ✦ Auto{/if}
		{#if batch.id === $lastBatch?.id}· <span class="latest">Latest</span>{/if}
	</span>
{/snippet}

{#if shown}
	<BatchSummary batch={shown} onback={() => (selected = null)} onclose={close} />
{:else}
	<Modal title="Batches" width="min(36rem, calc(100vw - 2rem))" onclose={close}>
		{#if error}
			<p class="empty error">{error}</p>
		{:else if batches === null}
			<p class="empty">Loading batches…</p>
		{:else if rows.length === 0}
			<p class="empty">No batches in this Anki profile.</p>
		{:else}
			<ul class="list">
				{#each groups as group (group.batches[0].id)}
					{#if group.batches.length === 1}
						{@const batch = group.batches[0]}
						<li>
							<button class="card" onclick={() => (selected = batch.id)}>
								<span class="title" title={batch.source.title}>{title(batch)}</span>
								<span class="when">{new Date(batch.started_at).toLocaleString()}</span>
								{@render meta(batch)}
								<span class="chevron" aria-hidden="true">›</span>
							</button>
						</li>
					{:else}
						{@const newest = group.batches[0]}
						{@const open = isOpen(group)}
						{@const notComplete = group.batches.filter((b) => unfinished(batchStatus(b))).length}
						{@const cards = group.batches.reduce((n, b) => n + b.items.length, 0)}
						<li class="group">
							<button class="card group-head" aria-expanded={open} onclick={() => toggle(group)}>
								<span class="title" title={group.show}
									><span class="caret" aria-hidden="true">{open ? '▾' : '▸'}</span>
									{group.show}</span
								>
								<span class="when">{new Date(newest.started_at).toLocaleDateString()}</span>
								<span class="meta">
									{plural(group.batches.length, 'batch', 'batches')} · {plural(cards, 'card')}
									{#if notComplete}·
										<span class="attention">{notComplete} not complete</span>{/if}
								</span>
							</button>
							{#if open}
								<ul class="episodes">
									{#each group.batches as batch (batch.id)}
										<li>
											<button
												class="episode"
												title={batch.source.title}
												onclick={() => (selected = batch.id)}
											>
												<span class="part">{episode(batch)}</span>
												{@render meta(batch)}
												<span class="when"
													>{sameDay(batch.started_at, newest.started_at)
														? new Date(batch.started_at).toLocaleTimeString()
														: new Date(batch.started_at).toLocaleString()}</span
												>
												<span class="chevron" aria-hidden="true">›</span>
											</button>
										</li>
									{/each}
								</ul>
							{/if}
						</li>
					{/if}
				{/each}
			</ul>
		{/if}
	</Modal>
{/if}

<style>
	.empty {
		margin: 0;
		padding: 0 1rem 1rem;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.error {
		color: var(--danger);
	}
	.list,
	.episodes {
		list-style: none;
		margin: 0;
		display: flex;
		flex-direction: column;
	}
	.list {
		padding: 0 1rem 1rem;
		gap: 0.3rem;
		overflow-y: auto;
	}
	.episodes {
		padding: 0.15rem 0 0.3rem 1.4rem;
	}
	.card {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.1rem 0.75rem;
		width: 100%;
		padding: 0.4rem 0.6rem;
		text-align: left;
		background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
		border: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
		border-radius: var(--radius);
	}
	.card:hover,
	.card:focus-visible {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.title {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: 0.9rem;
		color: var(--text);
	}
	.caret {
		display: inline-block;
		width: 1rem;
		color: var(--text-muted);
	}
	.when,
	.meta {
		font-size: 0.75rem;
		color: var(--text-muted);
	}
	.when {
		white-space: nowrap;
	}
	.card .meta {
		grid-column: 1 / 3;
	}
	.group-head .meta {
		padding-left: 1rem;
	}
	.chevron {
		color: var(--text-muted);
	}
	.card .chevron {
		grid-column: 3;
		grid-row: 1 / 3;
	}
	.episode {
		display: grid;
		grid-template-columns: 4.5rem minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.75rem;
		width: 100%;
		padding: 0.3rem 0.6rem;
		text-align: left;
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
	}
	.episode:hover,
	.episode:focus-visible {
		background: var(--bg-hover);
		border-color: var(--accent);
	}
	.part {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-size: 0.85rem;
		color: var(--text);
	}
	.episode .meta {
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}
	.attention {
		color: var(--warning);
	}
	.undone {
		color: var(--danger);
	}
	.latest {
		color: var(--accent);
	}
</style>
