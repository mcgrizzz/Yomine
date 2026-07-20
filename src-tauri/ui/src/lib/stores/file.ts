import { derived, get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError, overlay } from './ui';
import { languageToolsStatus } from './status';
import { refreshMinedState } from './mining';

/** The currently loaded file + its terms, or `null` before any file is opened. */
export const fileResult = writable<ipc.FileLoadResult | null>(null);

/** Gates the per-sentence comprehension bars — without Anki filtering every
 * sentence reads 0%. */
export const ankiFilterActive = derived(fileResult, ($f) => $f?.anki_filter_active ?? false);

export const recentFiles = writable<ipc.RecentFileEntry[]>([]);

/** Mirrors the engine's `SourceFileType::supported_extensions`. */
const SUPPORTED_EXTENSIONS = ['srt', 'ass', 'ssa', 'txt'];
export const isSupportedPath = (path: string): boolean => {
	const ext = path.split('.').pop()?.toLowerCase();
	return ext !== undefined && SUPPORTED_EXTENSIONS.includes(ext);
};

/** Errors surface as a banner without clobbering the currently-loaded file. */
export async function loadAndStore(path: string): Promise<void> {
	try {
		overlay.set('Processing file…');
		const result = await ipc.processFile(path, (msg) => overlay.set(msg.message));
		fileResult.set(result);
		void refreshMinedState(true);
		recentFiles.set(await ipc.getRecentFiles());
	} catch (err) {
		console.error('[yomine] process failed', err);
		lastError.set({ title: 'Failed to open file', message: String(err), detail: null });
	} finally {
		overlay.set(null);
	}
}

export async function openAndProcessFile(): Promise<void> {
	try {
		const path = await ipc.openFileDialog();
		if (!path) return;
		await loadAndStore(path);
	} catch (err) {
		console.error('[yomine] open dialog failed', err);
		lastError.set({ title: 'Failed to open file', message: String(err), detail: null });
	}
}

export function openRecentFile(path: string): Promise<void> {
	return loadAndStore(path);
}

export async function reloadCurrentFile(): Promise<void> {
	if (!get(fileResult)) return;
	try {
		overlay.set('Reprocessing file…');
		const result = await ipc.reloadCurrentFile((msg) => overlay.set(msg.message));
		fileResult.set(result);
		void refreshMinedState(true);
	} finally {
		overlay.set(null);
	}
}

/** The refreshed file lands via the `terms-refreshed` event, not the command result. */
export async function refreshTerms(): Promise<void> {
	if (get(languageToolsStatus) !== 'ready' || !get(fileResult)) return;
	try {
		overlay.set('Refreshing terms…');
		await ipc.refreshTerms();
	} catch (err) {
		lastError.set({ title: 'Refresh Error', message: 'Unable to refresh terms', detail: String(err) });
	} finally {
		overlay.set(null);
	}
}

export async function openDataFolder(): Promise<void> {
	try {
		await ipc.openDataFolder();
	} catch (err) {
		lastError.set({ title: 'Failed to open data folder', message: String(err), detail: null });
	}
}
