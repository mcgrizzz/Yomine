import { writable } from 'svelte/store';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import * as ipc from '$lib/ipc';
import { lastError, overlay, showNotice } from './ui';

export interface UpdateState {
	current: string;
	/** Display tag, e.g. "v0.6.1". */
	latest: string;
	url: string;
	/** True when the updater plugin can install it in-app (signed latest.json
	 * found); false = the pill just links to the release page. */
	installable: boolean;
}

export const updateInfo = writable<UpdateState | null>(null);

const RELEASES_PAGE = 'https://github.com/mcgrizzz/Yomine/releases/latest';

/** Held for installUpdate once the plugin check finds something. */
let pending: Update | null = null;

/** Plugin check first (signed latest.json); GitHub-API fallback when that's
 * missing or broken, so the pill can still link to the release page. Both
 * paths are best-effort — offline means no notice, never an error. */
export async function checkForUpdate(): Promise<void> {
	try {
		const update = await check();
		if (update) {
			pending = update;
			updateInfo.set({
				current: update.currentVersion,
				latest: `v${update.version}`,
				url: RELEASES_PAGE,
				installable: true
			});
			showNotice(`Yomine v${update.version} is available`);
		}
		return; // null = up to date
	} catch {
		// No latest.json on the newest release (or offline) — try the API fallback.
	}
	try {
		const u = await ipc.checkForUpdate();
		if (u) {
			updateInfo.set({ ...u, installable: false });
			showNotice(`Yomine ${u.latest} is available`);
		}
	} catch {
		// Offline / rate-limited — silently up-to-date.
	}
}

/** Download + install with overlay progress, then relaunch into the new version. */
export async function installUpdate(): Promise<void> {
	if (!pending) return;
	try {
		let total = 0;
		let done = 0;
		overlay.set('Downloading update…');
		await pending.downloadAndInstall((e) => {
			if (e.event === 'Started') {
				total = e.data.contentLength ?? 0;
			} else if (e.event === 'Progress') {
				done += e.data.chunkLength;
				overlay.set(
					total > 0
						? `Downloading update… ${Math.round((done / total) * 100)}%`
						: 'Downloading update…'
				);
			} else if (e.event === 'Finished') {
				overlay.set('Installing update…');
			}
		});
		await relaunch();
	} catch (err) {
		overlay.set(null);
		lastError.set({ title: 'Update failed', message: String(err), detail: null });
	}
}
