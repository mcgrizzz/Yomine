import { derived, get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { lastError, overlay, showNotice } from './ui';
import { ensureToolsReady, languageToolsStatus } from './status';
import { refreshMinedState } from './mining';
import { epubChapterModalOpen } from './modals';

/** The currently loaded file + its terms, or `null` before any file is opened. */
export const fileResult = writable<ipc.FileLoadResult | null>(null);

/** Gates the per-sentence comprehension bars — without Anki filtering every
 * sentence reads 0%. */
export const ankiFilterActive = derived(fileResult, ($f) => $f?.anki_filter_active ?? false);

export const recentFiles = writable<ipc.RecentFileEntry[]>([]);

/** The video local mining cuts the loaded file's media from. */
export const localVideo = derived(fileResult, ($f) => $f?.local_video ?? null);

/** A video is opened for local mining. Dynamic: a static './settings' import closes the
 * file.ts → settings.ts → controls.ts cycle (see locateMpvAndRetry in player.ts). */
async function useLocalMode(): Promise<void> {
	const { miningMode, setMiningMode } = await import('./settings');
	if (get(miningMode) === 'local') return;
	await setMiningMode('local');
	showNotice('Switched to Local: audio and screenshots come from the video');
}

/** Reloads the loaded video with another of its subtitle tracks. */
export async function switchSubtitleTrack(track: string): Promise<void> {
	const video = get(fileResult)?.local_video;
	if (!video) return;
	try {
		overlay.set('Loading subtitles…');
		fileResult.set(await ipc.openVideo(video, track, (msg) => overlay.set(msg.message)));
		void refreshMinedState(true);
	} catch (err) {
		lastError.set({ title: 'Failed to switch subtitles', message: String(err), detail: null });
	} finally {
		overlay.set(null);
	}
}

/** Picks a video for the loaded file; `unpair` removes the pairing instead. */
export async function pairVideo(unpair = false): Promise<void> {
	try {
		const path = unpair ? null : await ipc.openVideoDialog();
		if (!unpair && !path) return;
		const result = await ipc.pairVideo(path);
		if (result) fileResult.set(result);
	} catch (err) {
		lastError.set({ title: 'Pair video', message: String(err), detail: null });
	}
}

/** Mirror the engine's `SourceFileType::supported_extensions` and `VIDEO_EXTENSIONS`. */
const SUPPORTED_EXTENSIONS = ['srt', 'ass', 'ssa', 'txt', 'epub'];
const VIDEO_EXTENSIONS = ['mkv', 'mp4', 'avi', 'webm', 'mov', 'm4v', 'ts'];
const extensionOf = (path: string) => path.split('.').pop()?.toLowerCase() ?? '';
export const isVideoPath = (path: string): boolean => VIDEO_EXTENSIONS.includes(extensionOf(path));
export const isSupportedPath = (path: string): boolean =>
	SUPPORTED_EXTENSIONS.includes(extensionOf(path)) || isVideoPath(path);

/** The book behind the open chapter-picker modal, or `null`. */
export const epubPicker = writable<{ path: string; book: ipc.EpubBook } | null>(null);

async function openEpubPicker(path: string): Promise<void> {
	try {
		overlay.set('Reading EPUB…');
		const book = await ipc.getEpubChapters(path);
		epubPicker.set({ path, book });
		epubChapterModalOpen.set(true);
	} catch (err) {
		console.error('[yomine] epub listing failed', err);
		lastError.set({ title: 'Failed to open EPUB', message: String(err), detail: null });
	} finally {
		overlay.set(null);
	}
}

/** Errors surface as a banner without clobbering the currently-loaded file.
 * EPUBs detour through the chapter picker, which re-enters with a selection. */
export async function loadAndStore(
	path: string,
	epubChapters: number[] | null = null,
	epubLabel: string | null = null
): Promise<void> {
	if (!(await ensureToolsReady())) return;
	if (epubChapters === null && path.toLowerCase().endsWith('.epub')) {
		await openEpubPicker(path);
		return;
	}
	try {
		overlay.set('Processing file…');
		const result = isVideoPath(path)
			? await ipc.openVideo(path, null, (msg) => overlay.set(msg.message))
			: await ipc.processFile(path, (msg) => overlay.set(msg.message), epubChapters, epubLabel);
		fileResult.set(result);
		if (isVideoPath(path)) await useLocalMode();
		// Dynamic for the same import cycle as useLocalMode.
		if (result.local_video) void import('./auto').then((auto) => auto.onLocalVideo());
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
