import { sveltekit } from '@sveltejs/kit/vite';
import { execFileSync } from 'node:child_process';
import { defineConfig } from 'vite';

// @tauri-apps/cli sets TAURI_DEV_HOST when running on a physical device.
const host = process.env.TAURI_DEV_HOST;

let buildCommit = '';
try {
	buildCommit = execFileSync('git', ['rev-parse', 'HEAD'], {
		cwd: new URL('.', import.meta.url),
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'ignore']
	}).trim();
} catch {
	// Source archives may not contain Git metadata.
}

export default defineConfig({
	plugins: [sveltekit()],
	define: { __BUILD_COMMIT__: JSON.stringify(buildCommit) },

	// Pre-bundle wanakana (search normalization) so a fresh `pnpm install` is
	// picked up without a manual dev-server restart / re-optimization.
	optimizeDeps: {
		include: ['wanakana']
	},

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
	// esbuild 0.28+ treats destructuring as buggy below Safari 14.1 and errors
	// because it can't down-transpile SvelteKit's output; ship it natively like
	// the pre-vite-8 toolchain did (Safari 13 supports it, minus an obscure bug).
	esbuild: {
		supported: { destructuring: true }
	},
	build: {
		// Tauri uses Chromium on Windows and WebKit on macOS/Linux
		target: process.env.TAURI_ENV_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
		// don't minify for debug builds
		minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
		// produce sourcemaps for debug builds
		sourcemap: !!process.env.TAURI_ENV_DEBUG
	}
});
