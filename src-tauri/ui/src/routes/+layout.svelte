<script lang="ts">
	// Root layout (T026): loads the global theme and applies the dark/light variant
	// + serif toggle from the backend-owned settings. Settings hydrate shortly after
	// mount; until then the :root defaults (dark / sans) apply, so there's no flash.
	import '../app.css';
	import { settings } from '$lib/stores';

	let { children } = $props();

	$effect(() => {
		const s = $settings;
		document.documentElement.dataset.theme = s && !s.dark_mode ? 'light' : 'dark';
		document.body.classList.toggle('font-serif', s?.use_serif_font ?? false);
	});
</script>

{@render children()}
