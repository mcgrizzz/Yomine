import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// @tauri-apps/cli sets TAURI_DEV_HOST when running on a physical device.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [sveltekit()],

	// prevent Vite from obscuring Rust errors
	clearScreen: false,
	server: {
		// must match `devUrl` in tauri.conf.json
		port: 5173,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
					protocol: 'ws',
					host,
					port: 5183
				}
			: undefined,
		watch: {
			// don't watch the Rust side
			ignored: ['**/src-tauri/**']
		}
	},

	// expose Tauri env vars to the frontend
	envPrefix: ['VITE_', 'TAURI_ENV_*'],
	build: {
		// Tauri uses Chromium on Windows and WebKit on macOS/Linux
		target: process.env.TAURI_ENV_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
		// don't minify for debug builds
		minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
		// produce sourcemaps for debug builds
		sourcemap: !!process.env.TAURI_ENV_DEBUG
	}
});
