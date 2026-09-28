<script lang="ts">
	import { sanitize } from '$lib/sanitize';

	let { html }: { html: string } = $props();
</script>

<div class="glossary" lang="ja">
	<!-- eslint-disable-next-line svelte/no-at-html-tags -- sanitized -->
	{@html sanitize(html)}
</div>

<style>
	.glossary :global(ul),
	.glossary :global(ol) {
		margin: 0.2em 0;
		padding-left: 1.4em;
	}
	.glossary :global(img) {
		max-width: 100%;
	}
	/* The <i>(tags, Dictionary)</i> annotation Yomitan prefixes each sense with. */
	.glossary :global(.yomitan-glossary > i),
	.glossary :global(.yomitan-glossary ol > li > i) {
		color: var(--text-muted);
		font-size: 0.85em;
	}
	/* Mirrors the compact-glossary rules in Yomitan's structured-content.css:
	 * gloss alternatives inline, |-separated. */
	.glossary :global(ul[data-sc-content='glossary']),
	.glossary :global(.yomitan-glossary > ul),
	.glossary :global(.yomitan-glossary > ol > li > ul) {
		display: inline;
		margin: 0;
		padding-left: 0;
		list-style: none;
	}
	.glossary :global(ul[data-sc-content='glossary'] > li),
	.glossary :global(.yomitan-glossary > ul > li),
	.glossary :global(.yomitan-glossary > ol > li > ul > li) {
		display: inline;
	}
	.glossary :global(ul[data-sc-content='glossary'] > li:not(:first-child))::before,
	.glossary :global(.yomitan-glossary > ul > li:not(:first-child))::before,
	.glossary :global(.yomitan-glossary > ol > li > ul > li:not(:first-child))::before {
		content: ' | ';
		white-space: pre-wrap;
		color: var(--text-muted);
	}
</style>
