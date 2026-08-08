<script lang="ts">
	import { cachedEntry, entryLabel } from '$lib/definitions';
	import type { Term } from '$lib/ipc';
	import {
		adhocQueue,
		cardFormats,
		clearSelection,
		dropAdhoc,
		fileResult,
		queuedMineOptions,
		selectedTerms,
		setAdhocEntry,
		setAdhocFormat,
		setQueuedEntry,
		setQueuedFormat,
		settings,
		toggleSelected
	} from '$lib/stores';
	import { termKey } from '$lib/table';
	import DefinitionPopover from './DefinitionPopover.svelte';
	import Modal from './Modal.svelte';

	let { terms, onclose }: { terms: Term[]; onclose: () => void } = $props();

	const queueDetails = $derived.by(() => {
		const visible = new Set(terms.map(termKey));
		const rows = ($fileResult?.terms ?? terms)
			.filter((t) => $selectedTerms.has(termKey(t)))
			.map((t) => {
				const key = termKey(t);
				const opt = $queuedMineOptions[key];
				return {
					key,
					lemma: t.lemma_form,
					hidden: !visible.has(key),
					adhoc: false,
					formatName: opt?.formatName,
					entryIndex: opt?.entryIndex,
					scanText: opt?.scanText,
					entry: cachedEntry(opt?.scanText, opt?.entryIndex)
				};
			});
		return [
			...rows,
			...$adhocQueue.map((a) => ({
				key: a.key,
				lemma: a.lemma,
				hidden: false,
				adhoc: true,
				formatName: a.formatName,
				entryIndex: a.entryIndex,
				scanText: a.scanText,
				entry: cachedEntry(a.scanText, a.entryIndex)
			}))
		];
	});

	let entryPicker = $state<{
		key: string;
		adhoc: boolean;
		text: string;
		label: string;
		index?: number;
		anchor: DOMRect;
	} | null>(null);
</script>

<Modal title="Mining queue" width="min(620px, 94%)" {onclose}>
	<div class="queue-list">
		<div class="queue-row queue-head">
			<span>Term</span>
			<span>Entry</span>
			<span>Card format</span>
			<span></span>
		</div>
		{#each queueDetails as d (d.key)}
			<div class="queue-row">
				<span class="queue-term">
					<span lang="ja">{d.lemma}</span>
					{#if d.hidden}
						<span class="queue-dim">(hidden)</span>
					{:else if d.adhoc}
						<span class="queue-dim">(not in table)</span>
					{/if}
				</span>
				<span class="queue-entry">
					<span lang="ja">
						{#if d.entryIndex === undefined}
							<span class="queue-dim">Default</span>
						{:else if d.entry}
							{entryLabel(d.entry, d.lemma)}
						{:else}
							<span class="queue-dim">def #{d.entryIndex + 1}</span>
						{/if}
					</span>
					<!-- Without stopPropagation the popover's close-on-outside-click eats its own opening click. -->
					<button
						class="icon"
						aria-label={`Change the entry for ${d.lemma}`}
						title="Show the definitions and pick a different entry"
						onclick={(e) => {
							e.stopPropagation();
							entryPicker = {
								key: d.key,
								adhoc: d.adhoc,
								text: d.scanText ?? d.lemma,
								label: d.lemma,
								index: d.entryIndex,
								anchor: e.currentTarget.getBoundingClientRect()
							};
						}}>✎</button
					>
				</span>
				<span>
					{#if $cardFormats.length > 1}
						<select
							class="queue-select"
							value={d.formatName ?? $cardFormats[0].name}
							aria-label={`Card format for ${d.lemma}`}
							onchange={(e) =>
								d.adhoc
									? setAdhocFormat(d.key, e.currentTarget.value)
									: setQueuedFormat(d.key, e.currentTarget.value)}
						>
							{#each $cardFormats as f (f.name)}
								<option value={f.name}>{f.name}</option>
							{/each}
						</select>
					{:else}
						{$cardFormats[0]?.name ?? '—'}
					{/if}
				</span>
				<button
					class="icon remove"
					aria-label={`Remove ${d.lemma} from the queue`}
					onclick={() => (d.adhoc ? dropAdhoc(d.key) : toggleSelected(d.key))}>✕</button
				>
			</div>
		{/each}
	</div>
	{#snippet footer()}
		<footer class="queue-footer">
			<button onclick={clearSelection}>Clear all</button>
			<button class="right" onclick={onclose}>Close</button>
		</footer>
	{/snippet}
</Modal>

{#if entryPicker}
	{@const picker = entryPicker}
	<DefinitionPopover
		text={picker.text}
		label={picker.label}
		anchor={picker.anchor}
		scale={$settings?.definition_scale ?? 1}
		pickedIndex={picker.index}
		onpick={(entry) =>
			picker.adhoc
				? setAdhocEntry(picker.key, entry.index)
				: setQueuedEntry(picker.key, entry.index, picker.text)}
		onclose={() => (entryPicker = null)}
	/>
{/if}

<style>
	/* Rows subgrid these tracks (see TermTable's .table); a gap on .queue-row would be ignored. */
	.queue-list {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto auto;
		column-gap: 1rem;
	}
	.queue-row {
		grid-column: 1 / -1;
		display: grid;
		grid-template-columns: subgrid;
		align-items: center;
		padding: 0.45rem 1rem;
		border-bottom: 1px solid var(--border);
	}
	.queue-row:not(.queue-head):hover {
		background: var(--bg-raised);
	}
	.queue-head {
		position: sticky;
		top: 0;
		z-index: var(--z-sticky);
		background: var(--bg-panel);
		color: var(--text-muted);
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	.queue-term,
	.queue-entry {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 0 0.4rem;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.queue-dim {
		color: var(--text-muted);
		font-size: 0.9em;
	}
	.queue-select {
		max-width: 100%;
		font-size: 0.85rem;
	}
	.icon {
		padding: 0 0.25rem;
		background: none;
		border: none;
		color: var(--text);
		cursor: pointer;
	}
	.icon.remove {
		color: var(--danger);
	}
	.queue-footer {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0 1rem;
	}
	.queue-footer .right {
		margin-left: auto;
	}
</style>
