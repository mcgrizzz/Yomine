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

interface VideoQueue {
	videos: ipc.QueuedVideo[];
	index: number;
	/** Reached by Previous, Next or the list, so auto mode doesn't skip it when processed. */
	manual: boolean;
}

const videoQueue = writable<VideoQueue | null>(null);

/** The queue, while the loaded video is its current entry. */
export const currentQueue = derived([videoQueue, localVideo], ([$queue, $video]) =>
	$queue && $queue.videos[$queue.index].path === $video ? $queue : null
);

/** Opens the videos among `paths` as a queue; folders give their top-level videos. */
export async function openPaths(paths: string[]): Promise<void> {
	if (paths.length === 1 && isSupportedPath(paths[0])) return loadAndStore(paths[0]);
	let videos: ipc.QueuedVideo[];
	try {
		videos = await ipc.listVideos(paths);
	} catch (err) {
		lastError.set({ title: 'Failed to open', message: String(err), detail: null });
		return;
	}
	if (videos.length === 1) return loadAndStore(videos[0].path);
	if (videos.length > 1) return playQueue(videos, 0, false);
	const file = paths.find(isSupportedPath);
	if (file) return loadAndStore(file);
	showNotice('No videos found');
}

/** Loads `videos[from]`; an automatic move goes on past videos that fail to load. */
async function playQueue(videos: ipc.QueuedVideo[], from: number, manual: boolean): Promise<void> {
	const before = get(videoQueue);
	for (let index = from; index < videos.length; index++) {
		videoQueue.set({ videos, index, manual });
		if (await load(videos[index].path)) return;
		if (manual) break;
	}
	videoQueue.set(before);
}

export function goToQueued(index: number): Promise<void> {
	const queue = get(currentQueue);
	if (!queue || index < 0 || index >= queue.videos.length) return Promise.resolve();
	return playQueue(queue.videos, index, true);
}

export const closeQueue = (): void => videoQueue.set(null);

/** Moves on once auto mode is done with the video `fingerprint`. A processed video it
 * `skipped` is kept when the user moved to it. */
export async function advanceQueue(fingerprint: string, skipped = false): Promise<void> {
	const queue = get(currentQueue);
	if (!queue || (skipped && queue.manual)) return;
	if (get(fileResult)?.batch_source.fingerprint !== fingerprint) return;
	if (queue.index + 1 < queue.videos.length) return playQueue(queue.videos, queue.index + 1, false);
	showNotice(`Finished the queue of ${queue.videos.length} videos`);
}

/** Errors surface as a banner without clobbering the currently-loaded file.
 * EPUBs detour through the chapter picker, which re-enters with a selection. */
export async function loadAndStore(
	path: string,
	epubChapters: number[] | null = null,
	epubLabel: string | null = null
): Promise<void> {
	closeQueue();
	await load(path, epubChapters, epubLabel);
}

/** Whether `path` was loaded. */
async function load(
	path: string,
	epubChapters: number[] | null = null,
	epubLabel: string | null = null
): Promise<boolean> {
	if (!(await ensureToolsReady())) return false;
	if (epubChapters === null && path.toLowerCase().endsWith('.epub')) {
		await openEpubPicker(path);
		return false;
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
		return true;
	} catch (err) {
		console.error('[yomine] process failed', err);
		lastError.set({ title: 'Failed to open file', message: String(err), detail: null });
		return false;
	} finally {
		overlay.set(null);
	}
}

export async function openAndProcessFile(): Promise<void> {
	try {
		const paths = await ipc.openFileDialog();
		if (paths.length > 0) await openPaths(paths);
	} catch (err) {
		console.error('[yomine] open dialog failed', err);
		lastError.set({ title: 'Failed to open file', message: String(err), detail: null });
	}
}

export async function openFolder(): Promise<void> {
	try {
		const path = await ipc.openFolderDialog();
		if (path) await openPaths([path]);
	} catch (err) {
		lastError.set({ title: 'Failed to open folder', message: String(err), detail: null });
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
