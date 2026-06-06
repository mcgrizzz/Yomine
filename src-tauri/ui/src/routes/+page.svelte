<script lang="ts">
	// Phase B verification surface (T025): proves the full round trip — startup
	// loads language tools (progress overlay), then Open File → process_file →
	// a rendered term count. The real term table / sentence view land in Phase C.
	import { onMount } from 'svelte';
	import {
		hydrate,
		openAndProcessFile,
		languageToolsStatus,
		overlay,
		fileResult,
		ankiStatus
	} from '$lib/stores';

	onMount(hydrate);

	const toolsReady = $derived($languageToolsStatus === 'ready');
	const toolsError = $derived(
		typeof $languageToolsStatus === 'object' ? $languageToolsStatus.error : null
	);
</script>

<main>
	<header>
		<h1>Yomine</h1>
		<span class="anki" class:on={$ankiStatus.connected}>
			Anki: {$ankiStatus.connected ? 'connected' : 'offline'}
		</span>
	</header>

	{#if toolsError}
		<p class="error">Failed to load language tools: {toolsError}</p>
	{/if}

	<button onclick={openAndProcessFile} disabled={!toolsReady}>
		{toolsReady ? 'Open file…' : 'Loading tools…'}
	</button>

	{#if $fileResult}
		<section>
			<h2>{$fileResult.source_file.title}</h2>
			<p>
				<strong>{$fileResult.terms.length}</strong> minable terms ·
				{$fileResult.sentences.length} sentences ·
				{Math.round($fileResult.file_comprehension * 100)}% comprehension
			</p>
			<ul>
				{#each $fileResult.terms.slice(0, 50) as term (term.id)}
					<li>
						<span class="lemma">{term.lemma_form}</span>
						<span class="reading">{term.lemma_reading}</span>
						<span class="pos">{term.part_of_speech}</span>
						<span class="freq">#{term.frequencies.HARMONIC ?? '—'}</span>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if $overlay}
		<div class="overlay">{$overlay}</div>
	{/if}
</main>

<style>
	main {
		font-family: system-ui, sans-serif;
		padding: 2rem;
		max-width: 720px;
		margin: 0 auto;
	}
	header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
	}
	.anki {
		font-size: 0.85rem;
		color: #b45309;
	}
	.anki.on {
		color: #15803d;
	}
	.error {
		color: #b91c1c;
	}
	button {
		padding: 0.5rem 1rem;
		font-size: 1rem;
		cursor: pointer;
	}
	button:disabled {
		cursor: default;
		opacity: 0.6;
	}
	ul {
		list-style: none;
		padding: 0;
	}
	li {
		display: grid;
		grid-template-columns: 1fr 1fr auto auto;
		gap: 0.75rem;
		padding: 0.25rem 0;
		border-bottom: 1px solid #e5e7eb;
	}
	.reading {
		color: #6b7280;
	}
	.pos {
		color: #2563eb;
		font-size: 0.85rem;
	}
	.freq {
		color: #6b7280;
		font-variant-numeric: tabular-nums;
	}
	.overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(0, 0, 0, 0.55);
		color: white;
		font-size: 1.1rem;
	}
</style>
