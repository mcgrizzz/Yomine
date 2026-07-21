<script lang="ts">
	// Until settings hydrate, the :root defaults (dracula / sans) apply — no flash.
	import '../app.css';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { listen } from '@tauri-apps/api/event';
	import type { SettingsData } from '$lib/ipc';
	import { settings } from '$lib/stores';
	import { applyTheme, resolveTheme, type Theme } from '$lib/themes';

	let { children } = $props();


	onMount(() => {
		const unlisteners = [
			listen<SettingsData>('settings-changed', (e) => settings.set(e.payload)),
			listen<Theme | null>('theme-preview', (e) =>
				applyTheme(e.payload ?? resolveTheme(get(settings)))
			)
		];
		return () => unlisteners.forEach((p) => p.then((un) => un()));
	});

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
