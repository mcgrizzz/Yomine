import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),

	kit: {
		// SPA mode: prerender a single index.html fallback that Tauri serves as
		// static assets. No Node server ships (research R1).
		adapter: adapter({ fallback: 'index.html' })
	}
};

export default config;
