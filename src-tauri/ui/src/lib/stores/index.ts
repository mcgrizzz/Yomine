// Frontend stores (T024): components are pure functions of these. The backend is
// the single source of truth (Constitution); stores are a local mirror, hydrated
// on startup and kept live by the event subscriptions wired in `hydrate()`.

import { derived, get, writable } from 'svelte/store';
import * as ipc from '$lib/ipc';
import { applyControls, freqBounds, type SortDir, type SortField } from '$lib/table';

/** Tools lifecycle; the UI gates file actions on `ready`. */
export const languageToolsStatus = writable<ipc.LanguageToolsStatus>('loading');
export const ankiStatus = writable<ipc.AnkiStatus>({ connected: false, fetching: false });
export const playerStatus = writable<ipc.PlayerStatus>({
	mpv_connected: false,
	ws_clients: 0,
	mode: 'none'
});

/** Whether a player is connected (mpv or an asbplayer ws client) — gates the
 * clickable timestamp seek, mirroring egui `Player::is_connected`. */
export const playerConnected = derived(
	playerStatus,
	($p) => $p.mpv_connected || $p.ws_clients > 0
);

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

// ---- Term-table controls (T037): client-side search / sort / POS / frequency.
// Components read `visibleTerms`; the controls write these. -------------------

/** Free-text query (term forms, readings, POS, and sentence text). */
export const tableSearch = writable('');

/** Active sort column + direction; defaults to egui's frequency-ascending. */
export const tableSort = writable<{ field: SortField; dir: SortDir }>({
	field: 'frequency',
	dir: 'asc'
});

/** POS-key → enabled. Seeded from `settings.pos_filters`; missing key = enabled. */
export const posEnabled = writable<Record<string, boolean>>({});

/** Frequency filter: `lo`/`hi` are the data bounds (slider extent), `min`/`max` the selection. */
export interface FreqFilterState {
	lo: number;
	hi: number;
	min: number;
	max: number;
	includeUnknown: boolean;
}
export const freqFilter = writable<FreqFilterState | null>(null);

// Re-derive the frequency bounds whenever the term set changes (new file or an
// Anki refresh), mirroring egui's `configure_bounds`. Selection resets to the
// full range; unknown-frequency terms start hidden (egui's `include_unknown=false`),
// revealed via the "?" toggle.
fileResult.subscribe((r) => {
	if (!r) {
		freqFilter.set(null);
		return;
	}
	const { min, max } = freqBounds(r.terms);
	freqFilter.set({ lo: min, hi: max, min, max, includeUnknown: false });
});

/** The filtered + sorted term list the table renders (pure function of the controls). */
export const visibleTerms = derived(
	[fileResult, tableSearch, tableSort, posEnabled, freqFilter],
	([$file, $search, $sort, $pos, $freq]) =>
		$file
			? applyControls($file.terms, $file.sentences, {
					search: $search,
					sort: $sort,
					pos: $pos,
					freq: $freq
				})
			: []
);

// ---- Ignore list (T038 / T038b) --------------------------------------------

/** Whether the ignore-list modal is open. The modal owns its staged term/file
 * state (egui's `temp_terms`/`temp_files`) and self-hydrates via `getIgnoreListFull`. */
export const ignoreModalOpen = writable(false);

/** Open the full ignore-list modal (it hydrates its own staged state on open). */
export function openIgnoreModal(): void {
	ignoreModalOpen.set(true);
}

/**
 * Add a term's lemma to the ignore list (a row's right-click action). This is the
 * one *immediate* ignore path (egui parity); the modal stages and persists on save.
 * The backend re-filters and returns the updated file; if it does, the table updates.
 */
export async function addToIgnore(lemma: string): Promise<void> {
	const result = await ipc.addToIgnoreList(lemma);
	if (result) fileResult.set(result);
}

/**
 * Persist the modal's staged terms + files (the single commit point). The backend
 * re-filters and returns the updated file; if it does, the table updates in place.
 */
export async function saveIgnore(terms: string[], files: ipc.IgnoreFile[]): Promise<void> {
	const result = await ipc.saveIgnoreList(terms, files);
	if (result) fileResult.set(result);
}

// ---- Top-bar theme / font toggles (T028) -----------------------------------
// Mirror egui's top-bar ☀/🌙 + 字 buttons: flip the bit, mirror it locally so the
// root layout re-applies the theme/font immediately, then persist (egui's
// `request_save_settings`). No-op until settings have hydrated.

/** Toggle dark/light mode and persist (the ☀/🌙 button). */
export async function toggleDarkMode(): Promise<void> {
	const s = get(settings);
	if (!s) return;
	const updated = { ...s, dark_mode: !s.dark_mode };
	settings.set(updated);
	await ipc.saveSettings(updated);
}

/** Toggle the serif/sans font family and persist (the 字 button). */
export async function toggleSerifFont(): Promise<void> {
	const s = get(settings);
	if (!s) return;
	const updated = { ...s, use_serif_font: !s.use_serif_font };
	settings.set(updated);
	await ipc.saveSettings(updated);
}

/** Last surfaced error (for a modal); `null` once dismissed. */
export const lastError = writable<ipc.ErrorPayload | null>(null);

/** Seek the connected player to a sentence timestamp (US3/T035). Mirrors egui's
 * SeekTimestamp action; surfaces failures via `lastError` instead of eprintln. */
export async function seekTimestamp(seconds: number, label: string): Promise<void> {
	try {
		await ipc.seekTimestamp(seconds, label);
	} catch (err) {
		lastError.set({ title: 'Failed to seek', message: String(err), detail: null });
	}
}

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
	posEnabled.set({ ...loadedSettings.pos_filters });
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
