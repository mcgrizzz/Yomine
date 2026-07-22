import { get, writable } from 'svelte/store';
import type * as ipc from '$lib/ipc';
import { initProgress, lastError, overlay } from './ui';

/** Tools lifecycle; the UI gates file actions on `ready`. */
export const languageToolsStatus = writable<ipc.LanguageToolsStatus>('loading');

function awaitToolsReady(): Promise<void> {
	return new Promise((resolve, reject) => {
		let settled = false;
		const unsub = languageToolsStatus.subscribe((s) => {
			if (settled || s === 'loading') return;
			settled = true;
			if (s === 'ready') resolve();
			else reject(new Error(s.error));
			// The first callback fires before `unsub` is assigned.
			queueMicrotask(() => unsub());
		});
	});
}

/** True once the tools are ready, escalating init progress to the blocking
 * overlay while waiting; false when init failed (surfaced via `lastError`). */
export async function ensureToolsReady(): Promise<boolean> {
	if (get(languageToolsStatus) === 'ready') return true;
	const unsub = initProgress.subscribe((m) => overlay.set(m ?? 'Loading language tools…'));
	try {
		await awaitToolsReady();
		return true;
	} catch (err) {
		lastError.set({
			title: 'Language Tools',
			message: 'Language tools failed to load',
			detail: String(err)
		});
		return false;
	} finally {
		unsub();
		overlay.set(null);
	}
}

export const ankiStatus = writable<ipc.AnkiStatus>({ connected: false, fetching: false });

export const knowledge = writable<ipc.KnowledgeSummary | null>(null);
