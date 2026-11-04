<script lang="ts">
	import Modal from './Modal.svelte';
	import type { MinedBefore } from '$lib/ipc';
	import {
		answerMinedBefore,
		autoMinedBefore,
		autoMode,
		currentQueue,
		fileResult,
		minedBeforeSummary,
		playerBusy
	} from '$lib/stores';

	let { found }: { found: MinedBefore } = $props();

	let always = $state(false);
	const verb = $derived($autoMode === 'review' ? 'Review' : 'Mine');
	const hasNext = $derived(
		$currentQueue !== null && $currentQueue.index + 1 < $currentQueue.videos.length
	);
</script>

<Modal
	title="Already mined this file"
	width="min(30rem, calc(100vw - 2rem))"
	onclose={() => autoMinedBefore.set(null)}
>
	<div class="body">
		<strong class="title">{$fileResult?.source_file.title}</strong>
		<p class="muted">{minedBeforeSummary(found)}.</p>
		<p>{verb} it again anyway?</p>
		<label class="always">
			<input type="checkbox" bind:checked={always} />
			Do the same for the rest of this session
		</label>
	</div>
	{#snippet footer()}
		<footer>
			<button
				class="primary"
				disabled={$playerBusy}
				onclick={() => void answerMinedBefore(true, always)}>{verb} anyway</button
			>
			<span class="spacer"></span>
			<button onclick={() => void answerMinedBefore(false, always)}
				>{hasNext ? 'Skip to the next video' : 'Skip'}</button
			>
		</footer>
	{/snippet}
</Modal>

<style>
	.body {
		display: grid;
		gap: 0.4rem;
		padding: 0.25rem 1rem 0.75rem;
	}
	.body p {
		margin: 0;
	}
	.title {
		overflow-wrap: anywhere;
	}
	.muted {
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.always {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		margin-top: 0.3rem;
		font-size: 0.85rem;
	}
	footer {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem 1rem 0;
		border-top: 1px solid var(--border);
	}
	.spacer {
		flex: 1;
	}
	footer .primary {
		font-weight: 600;
		color: var(--bg-deep);
		background: var(--accent);
		border-color: var(--accent);
	}
</style>
