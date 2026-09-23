<script lang="ts">
	let { retryLabel }: { retryLabel: string } = $props();
</script>

<ol>
	<li>Switch to the video's tab in your browser.</li>
	<li>
		Click the asbplayer button in the browser toolbar. If it shows <em>Enable audio recording</em>,
		allow it.
	</li>
	<li>Come back here and choose <strong>{retryLabel}</strong>.</li>
</ol>
<p class="aside">
	Recording must be enabled once for each new tab. If it already is, check asbplayer's Anki settings
	(deck, note type, and field mappings).
</p>

<style>
	ol {
		margin: 0;
		padding-left: 1.3rem;
		line-height: 1.55;
	}
	li + li {
		margin-top: 0.2rem;
	}
	.aside {
		margin: 0.5rem 0 0;
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.45;
	}
</style>
