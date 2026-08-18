<script lang="ts">
	// Until settings hydrate, the :root defaults (dracula / sans) apply — no flash.
	import '../app.css';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { listen } from '@tauri-apps/api/event';
	import * as ipc from '$lib/ipc';
	import type { SettingsData, UserTheme } from '$lib/ipc';
	import { settings, userThemes } from '$lib/stores';
	import { applyTheme, resolveTheme, type Theme } from '$lib/themes';

	let { children } = $props();

	// Both windows render this layout, so the theme library loads once here, not per-window.
	onMount(() => {
		void ipc.getUserThemes().then((t) => userThemes.set(t));
		const unlisteners = [
			listen<SettingsData>('settings-changed', (e) => settings.set(e.payload)),
			listen<UserTheme[]>('user-themes-changed', (e) => userThemes.set(e.payload)),
			listen<Theme | null>('theme-preview', (e) =>
				applyTheme(e.payload ?? resolveTheme(get(settings), get(userThemes)))
			)
		];
		return () => unlisteners.forEach((p) => p.then((un) => un()));
	});

	$effect(() => {
		const s = $settings;
		if ($userThemes !== null) applyTheme(resolveTheme(s, $userThemes));
		document.body.classList.toggle('font-serif', s?.use_serif_font ?? false);
		// CSS zoom (not root font-size) so px-based sizes scale too; the
		// Appearance modal live-previews by setting this same property.
		document.documentElement.style.setProperty('zoom', String(s?.font_scale ?? 1));
	});
</script>

{@render children()}
