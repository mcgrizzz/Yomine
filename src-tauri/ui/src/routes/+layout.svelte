<script lang="ts">
	// Until settings hydrate, the :root defaults (dracula / sans) apply — no flash.
	import '../app.css';
	import { settings } from '$lib/stores';
	import { applyTheme, resolveTheme } from '$lib/themes';

	let { children } = $props();

	$effect(() => {
		const s = $settings;
		applyTheme(resolveTheme(s));
		document.body.classList.toggle('font-serif', s?.use_serif_font ?? false);
		// CSS zoom (not root font-size) so px-based sizes scale too; the
		// Appearance modal live-previews by setting this same property.
		document.documentElement.style.setProperty('zoom', String(s?.font_scale ?? 1));
	});
</script>

{@render children()}
