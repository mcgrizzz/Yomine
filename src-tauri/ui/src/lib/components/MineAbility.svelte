<script lang="ts">
	let { note, media }: { note: boolean; media: boolean } = $props();
</script>

{#snippet slash()}
	<svg class="slash" viewBox="0 0 16 16" aria-hidden="true">
		<path d="M2 14L14 2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
	</svg>
{/snippet}

<span class="ability">
	<span
		class="icon"
		class:off={!note}
		role="img"
		aria-label={note ? 'Creates the note' : "Can't create the note"}
		title={note ? 'Mining creates the note' : "Mining can't create the note: Anki or Yomitan isn't reachable"}
	>
		<svg viewBox="0 0 16 16" aria-hidden="true">
			<rect x="2" y="3" width="12" height="10" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.5" />
			<path d="M5 6.5h6M5 9.5h4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
		</svg>
		{#if !note}{@render slash()}{/if}
	</span>
	<span
		class="icon"
		class:off={!media}
		role="img"
		aria-label={media ? 'Adds screenshot and audio' : 'No screenshot or audio'}
		title={media ? 'Mining adds the screenshot and audio' : 'Mining adds no screenshot or audio'}
	>
		<svg viewBox="0 0 16 16" aria-hidden="true">
			<rect x="1.5" y="3" width="13" height="10" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.5" />
			<circle cx="5.2" cy="6.3" r="1.2" fill="currentColor" />
			<path d="M2.5 12l3.6-3.4 2.4 2.2 2.2-1.8 3 2.6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
		</svg>
		{#if !media}{@render slash()}{/if}
	</span>
</span>

<style>
	.ability {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
	}
	.icon {
		position: relative;
		display: inline-grid;
		place-items: center;
		width: 1.1rem;
		height: 1.1rem;
		color: var(--success);
		cursor: help;
	}
	.icon > svg {
		grid-area: 1 / 1;
		width: 1rem;
		height: 1rem;
	}
	.icon.off {
		color: var(--text-muted);
	}
	.icon .slash {
		color: var(--danger);
	}
</style>
