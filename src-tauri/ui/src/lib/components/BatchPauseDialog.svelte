<script lang="ts">
	import Modal from './Modal.svelte';
	import RecordingSteps from './RecordingSteps.svelte';
	import type { BatchPauseState, PauseChoice } from '$lib/stores/batches';

	interface Props {
		pause: BatchPauseState;
		choices: PauseChoice[];
		onchoose: (choice: PauseChoice) => void;
	}

	let { pause, choices, onchoose }: Props = $props();

	const created = $derived(pause.item.outcome.status === 'created');
	const shared = $derived(pause.failure.scope === 'shared');
	const unrecorded = $derived(pause.failure.kind === 'media_unverified');
	const headline = $derived(
		unrecorded
			? "asbplayer didn't record this card"
			: created
				? "The card's media wasn't added"
				: shared
					? "Cards can't be created right now"
					: "This card wasn't created"
	);

	function describe(choice: PauseChoice): { label: string; detail: string } {
		switch (choice) {
			case 'retry':
				if (created) {
					return { label: 'Retry media', detail: 'Record audio and a screenshot for this card again.' };
				}
				if (shared) {
					return { label: 'Retry after fixing', detail: 'Try this card again once the problem is fixed.' };
				}
				return { label: 'Retry card', detail: 'Try creating this card again.' };
			case 'without_dictionary_media':
				return {
					label: 'Create without dictionary media',
					detail: "Create this card without the Yomitan audio or images Anki couldn't save."
				};
			case 'skip':
				if (created) {
					return { label: 'Skip media for this card', detail: 'Keep the card as is and record the rest.' };
				}
				return { label: 'Skip this card', detail: 'Leave it uncreated and continue with the next card.' };
			case 'stop':
				return { label: 'Stop batch', detail: '' };
		}
	}
</script>

<Modal
	title="Batch paused"
	width="min(34rem, calc(100vw - 2rem))"
	dismissible={false}
	onclose={() => onchoose('stop')}
>
	<div class="body">
		<div class="card">
			<strong class="word" lang="ja">{pause.item.lemma}</strong>
			<span class="chip" class:ok={created}>{created ? 'Card created' : 'Card not created'}</span>
		</div>
		<p class="sentence" lang="ja">{pause.item.sentence}</p>

		<h3>{headline}</h3>
		{#if unrecorded}
			<RecordingSteps retryLabel="Retry media" />
		{:else}
			<p class="message">{pause.failure.message}</p>
		{/if}
		{#if shared}
			<p class="shared">This affects the remaining cards too, so skipping isn't offered.</p>
		{/if}

		<div class="choices" role="group" aria-label="How to continue">
			{#each choices.filter((c) => c !== 'stop') as choice (choice)}
				{@const option = describe(choice)}
				<button class="choice" class:primary={choice === 'retry'} onclick={() => onchoose(choice)}>
					<span class="label">{option.label}</span>
					<span class="detail">{option.detail}</span>
				</button>
			{/each}
		</div>

		<details>
			<summary>Technical details</summary>
			<p class="raw"><strong>{pause.failure.stage}:</strong> {pause.failure.message}</p>
		</details>
	</div>
	{#snippet footer()}
		<footer>
			<span class="muted">
				Cards already created are kept. Retry media can add missing recordings later.
			</span>
			<button onclick={() => onchoose('stop')}>Stop batch</button>
		</footer>
	{/snippet}
</Modal>

<style>
	.body {
		padding: 0.25rem 1rem 0.5rem;
	}
	.card {
		display: flex;
		align-items: center;
		gap: 0.6rem;
	}
	.word {
		font-size: 1.2rem;
	}
	.chip {
		padding: 0.1rem 0.55rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-pill);
		color: var(--text-muted);
		font-size: 0.75rem;
	}
	.chip.ok {
		color: var(--success);
		border-color: color-mix(in srgb, var(--success) 45%, var(--border));
	}
	.sentence {
		margin: 0.25rem 0 0;
		color: var(--text-muted);
		font-size: 0.85rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	h3 {
		margin: 1rem 0 0.4rem;
		font-size: 1.1rem;
	}
	.message {
		margin: 0;
		line-height: 1.5;
		overflow-wrap: anywhere;
	}
	.shared {
		margin: 0.5rem 0 0;
		color: var(--warning);
		font-size: 0.85rem;
	}
	.choices {
		display: grid;
		gap: 0.5rem;
		margin-top: 1rem;
	}
	.choice {
		display: grid;
		gap: 0.2rem;
		padding: 0.6rem 0.8rem;
		text-align: left;
		white-space: normal;
	}
	.choice.primary {
		border-color: var(--accent);
		background: color-mix(in srgb, var(--accent) 12%, var(--bg-raised));
	}
	.label {
		font-weight: 600;
	}
	.detail {
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.4;
	}
	details {
		margin-top: 0.9rem;
		color: var(--text-muted);
		font-size: 0.8rem;
	}
	summary {
		width: fit-content;
		cursor: pointer;
	}
	.raw {
		margin: 0.4rem 0 0;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.75rem 1rem 0;
		border-top: 1px solid var(--border);
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.8rem;
	}
</style>
