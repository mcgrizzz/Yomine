<script lang="ts">
	let { match }: { match: string } = $props();
	const tooltipId = $props.id();
	let root: HTMLSpanElement;
	let open = $state(false);
</script>

<svelte:window
	onpointerdown={(event) => {
		if (!root.contains(event.target as Node)) open = false;
	}}
	onkeydown={(event) => { if (event.key === 'Escape') open = false; }}
/>

<span
	class="uncertainty"
	bind:this={root}
	role="presentation"
	onmouseenter={() => (open = true)}
	onmouseleave={() => (open = false)}
>
	<button
		type="button"
		class="trigger"
		aria-label="Uncertain Anki match"
		aria-describedby={tooltipId}
		onfocus={() => (open = true)}
		onblur={() => (open = false)}
		onclick={() => (open = true)}
		onkeydown={(event) => {
			if (event.key === 'Enter' || event.key === ' ') event.stopPropagation();
		}}
	>
		<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true">
			<circle cx="12" cy="12" r="9" />
			<path d="M9.5 9a2.5 2.5 0 0 1 5 .5c0 1.7-2.5 2-2.5 3.5" />
			<path d="M12 16v.1" stroke-linecap="round" />
		</svg>
	</button>
	<span id={tooltipId} role="tooltip" class="explanation" hidden={!open}>
		Possible Anki match: <strong lang="ja">{match}</strong>.
		This word is still available to mine and is not counted as known.
	</span>
</span>

<style>
	.uncertainty { position: relative; display: inline-flex; flex: none; font-size: 0.8rem; }
	.trigger { display: flex; align-items: center; justify-content: center; width: 1.4rem; height: 1.4rem; padding: 0; border: 0; background: transparent; color: var(--text-muted); cursor: help; border-radius: 50%; }
	.trigger:hover, .trigger:focus-visible { color: var(--accent); background: var(--bg-raised); }
	.trigger:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
	.explanation { position: absolute; left: 0; top: 100%; z-index: var(--z-popover); width: 15rem; max-width: 65vw; padding: 0.65rem 0.8rem; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-raised); color: var(--text); box-shadow: 0 3px 12px #0004; line-height: 1.5; white-space: normal; cursor: auto; }
</style>
