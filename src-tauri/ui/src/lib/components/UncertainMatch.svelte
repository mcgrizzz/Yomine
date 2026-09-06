<script lang="ts">
	let { match }: { match: string } = $props();
</script>

<details class="uncertainty">
	<summary onkeydown={(event) => { if (['Escape', ' ', 'Enter'].includes(event.key)) event.stopPropagation(); if (event.key === 'Escape') event.currentTarget.closest('details')?.removeAttribute('open'); }} aria-label={`Uncertain Anki match: ${match}`} title={`Possible Anki match: ${match}. Click for details.`}>
		<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true">
			<circle cx="12" cy="12" r="9" />
			<path d="M9.5 9a2.5 2.5 0 0 1 5 .5c0 1.7-2.5 2-2.5 3.5" />
			<path d="M12 16v.1" stroke-linecap="round" />
		</svg>
	</summary>
	<span class="explanation">
		Possible Anki match: <strong lang="ja">{match}</strong>.
		This word is still available to mine and is not counted as known.
	</span>
</details>

<style>
	.uncertainty { position: relative; display: inline-block; flex: none; font-size: 0.8rem; }
	summary { display: flex; align-items: center; justify-content: center; width: 1.4rem; height: 1.4rem; color: var(--text-muted); cursor: pointer; list-style: none; border-radius: 50%; }
	summary::-webkit-details-marker { display: none; }
	summary:hover, .uncertainty[open] summary { color: var(--accent); background: var(--bg-raised); }
	summary:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
	.explanation { position: absolute; left: 0; top: calc(100% + 0.35rem); z-index: var(--z-popover); width: 15rem; max-width: 65vw; padding: 0.65rem 0.8rem; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-raised); color: var(--text); box-shadow: 0 3px 12px #0004; line-height: 1.5; white-space: normal; }
</style>
