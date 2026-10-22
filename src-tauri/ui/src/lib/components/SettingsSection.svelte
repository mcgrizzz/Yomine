<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, children }: { title: string; children: Snippet } = $props();
	const id = $props.id();
</script>

<section aria-labelledby={id}>
	<h4 {id}>{title}</h4>
	{@render children()}
</section>

<style>
	section {
		display: grid;
		gap: 0.6rem;
	}
	h4 {
		margin: 0;
		font-size: 0.85rem;
		font-weight: 600;
	}
</style>
