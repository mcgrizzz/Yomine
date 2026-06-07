// Frontend stores (T024): components are pure functions of these. The backend is
// the single source of truth (Constitution); stores are a local mirror, hydrated
// on startup and kept live by the event subscriptions wired in `hydrate()`.

import { get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';

/** Tools lifecycle; the UI gates file actions on `ready`. */
export const languageToolsStatus = writable<ipc.LanguageToolsStatus>('loading');
export const ankiStatus = writable<ipc.AnkiStatus>({ connected: false, fetching: false });
export const playerStatus = writable<ipc.PlayerStatus>({
	mpv_connected: false,
	ws_clients: 0,
	mode: 'none'
});

/** Loading overlay text (`null` = hidden), mirroring egui's MessageOverlay. */
export const overlay = writable<string | null>(null);

/** The currently loaded file + its terms, or `null` before any file is opened. */
export const fileResult = writable<ipc.FileLoadResult | null>(null);

/** Recently-opened files for the landing state; refreshed after each load. */
export const recentFiles = writable<ipc.RecentFileEntry[]>([]);

/** True while a supported file is being dragged over the window (drop overlay). */
export const dragHovering = writable(false);

/** Extensions the pipeline accepts (mirrors `SourceFileType::supported_extensions`). */
const SUPPORTED_EXTENSIONS = ['srt', 'ass', 'ssa', 'txt'];
const isSupportedPath = (path: string): boolean => {
	const ext = path.split('.').pop()?.toLowerCase();
	return ext !== undefined && SUPPORTED_EXTENSIONS.includes(ext);
};

export const knowledge = writable<ipc.KnowledgeSummary | null>(null);
export const settings = writable<ipc.SettingsData | null>(null);
export const posCatalog = writable<ipc.PosInfo[]>([]);

/** Last surfaced error (for a modal); `null` once dismissed. */
export const lastError = writable<ipc.ErrorPayload | null>(null);

let hydrated = false;

/**
 * Subscribe to all ambient events, hydrate the stores from the backend, then kick
 * off tool loading. Idempotent — safe to call once from the root layout/page.
 */
export async function hydrate(): Promise<void> {
	if (hydrated) return;
	hydrated = true;

	// Live event wiring (set up before any await so we don't miss early emits).
	ipc.onLanguageToolsStatus((s) => languageToolsStatus.set(s));
	ipc.onAnkiStatus((s) => ankiStatus.set(s));
	ipc.onPlayerStatus((s) => playerStatus.set(s));
	ipc.onKnowledgeSummary((s) => knowledge.set(s));
	ipc.onTermsRefreshed((r) => fileResult.set(r));
	ipc.onError((e) => lastError.set(e));

	// Native drag-drop: show a "drop to open" overlay while a supported file hovers,
	// and load the first supported file on drop (egui parity). Ignored entirely until
	// the language tools finish loading — there's nothing to process a file with yet.
	const toolsReady = () => get(languageToolsStatus) === 'ready';
	ipc.onDragDrop({
		onEnter: (paths) => dragHovering.set(toolsReady() && paths.some(isSupportedPath)),
		onDrop: (paths) => {
			dragHovering.set(false);
			if (!toolsReady()) return;
			const file = paths.find(isSupportedPath);
			if (file) loadAndStore(file);
		},
		onLeave: () => dragHovering.set(false)
	});

	// Hydrate from the commands that exist today; the rest arrive via events.
	const [loadedSettings, catalog, currentFile, recents] = await Promise.all([
		ipc.getSettings(),
		ipc.getPosCatalog(),
		ipc.getTerms(),
		ipc.getRecentFiles()
	]);
	settings.set(loadedSettings);
	posCatalog.set(catalog);
	fileResult.set(currentFile);
	recentFiles.set(recents);

	// Begin loading the heavy language tools; progress streams to the overlay.
	overlay.set('Loading language tools…');
	try {
		await ipc.loadLanguageTools((msg) => overlay.set(msg.message));
	} catch (err) {
		languageToolsStatus.set({ error: String(err) });
	} finally {
		overlay.set(null);
	}
}

/**
 * Process a file by path and store the result. Used by the open dialog, recent
 * files, and drag-drop. Errors surface as a banner without clobbering the
 * currently-loaded file.
 */
async function loadAndStore(path: string): Promise<void> {
	try {
		overlay.set('Processing file…');
		const result = await ipc.processFile(path, (msg) => overlay.set(msg.message));
		fileResult.set(result);
		recentFiles.set(await ipc.getRecentFiles());
	} catch (err) {
		console.error('[yomine] process failed', err);
		lastError.set({ title: 'Failed to open file', message: String(err), detail: null });
	} finally {
		overlay.set(null);
	}
}

/** Open the native dialog, process the chosen file, and store the result. */
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

/** Open a previously-loaded file directly from the recent-files list. */
export function openRecentFile(path: string): Promise<void> {
	return loadAndStore(path);
}
