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
		// Whole-UI scale (Appearance modal): CSS zoom scales px sizes too, unlike
		// a root font-size change. The modal live-previews by setting this directly.
		document.documentElement.style.setProperty('zoom', String(s?.font_scale ?? 1));
	});
</script>

{@render children()}
