// Typed IPC layer (T023): thin wrappers over Tauri `invoke` / `listen` / `Channel`,
// plus the wire types that cross the boundary (data-model.md). This is the only
// module that imports `@tauri-apps/api`; everything else talks to these helpers.

import { invoke, Channel } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';

// ---------------------------------------------------------------------------
// Wire types (mirror specs/001-tauri-port/data-model.md)
// ---------------------------------------------------------------------------

/** POS variant key (`POS::as_key()`), e.g. "Noun", "Verb". */
export type Pos = string;

export interface Term {
	id: number;
	lemma_form: string;
	lemma_reading: string;
	surface_form: string;
	surface_reading: string;
	is_kana: boolean;
	part_of_speech: Pos;
	/** dictionary id → rank; includes the combined `"HARMONIC"` key. */
	frequencies: Record<string, number>;
	full_segment: string;
	full_segment_reading: string;
	/** (sentence_id, start_index) pairs. */
	sentence_references: [number, number][];
	comprehension: number;
}

export interface SegmentDto {
	/** Pre-sliced surface text for this span. */
	surface: string;
	/** Reading in hiragana (furigana). */
	reading: string;
	pos: Pos;
	/** UTF-8 byte offsets into the sentence text (for the term-overlap test). */
	start: number;
	end: number;
}

export interface TimeStampDto {
	start_secs: number;
	end_secs: number;
	start_label: string;
	end_label: string;
}

export interface SentenceDto {
	id: number;
	source_id: number;
	text: string;
	segments: SegmentDto[];
	timestamp: TimeStampDto | null;
	comprehension: number;
}

export type SourceFileType = 'SRT' | 'SSA' | 'TXT' | { Other: string };

export interface SourceFile {
	id: number;
	source: string | null;
	file_type: SourceFileType;
	title: string;
	creator: string | null;
	original_file: string;
}

export interface FileLoadResult {
	source_file: SourceFile;
	terms: Term[];
	sentences: SentenceDto[];
	file_comprehension: number;
}

/** A previously-opened file for the landing state (mirrors `RecentFileEntry`). */
export interface RecentFileEntry {
	file_path: string;
	title: string;
	creator: string | null;
	/** RFC3339 timestamp. */
	last_opened: string;
	file_size: number | null;
	term_count: number | null;
}

export interface PosInfo {
	key: string;
	display_name: string;
}

export interface FieldMapping {
	term_field: string;
	reading_field: string;
}

export interface FrequencyDictionarySetting {
	weight: number;
	enabled: boolean;
}

export interface SettingsData {
	anki_model_mappings: Record<string, FieldMapping>;
	anki_interval: number;
	websocket_settings: { port: number };
	frequency_weights: Record<string, FrequencyDictionarySetting>;
	pos_filters: Record<string, boolean>;
	use_serif_font: boolean;
	dark_mode: boolean;
}

export interface BandStats {
	coverage: number;
	comprehension: number;
	total: number;
}

export interface KnowledgeSummary {
	jlpt: { level: string; stats: BandStats }[];
	frequency: { label: string; stats: BandStats }[];
}

// ---- Event payloads (contracts/events.md) ----

export interface LoadingMessage {
	message: string | null;
}

/** Serde-tagged: `"loading"` | `"ready"` | `{ error: string }`. */
export type LanguageToolsStatus = 'loading' | 'ready' | { error: string };

export interface AnkiStatus {
	connected: boolean;
	fetching: boolean;
}

export interface PlayerStatus {
	mpv_connected: boolean;
	ws_clients: number;
	mode: 'mpv' | 'asbplayer' | 'none';
}

export interface ErrorPayload {
	title: string;
	message: string;
	detail: string | null;
}

// ---------------------------------------------------------------------------
// Commands (only those registered in the backend invoke_handler)
// ---------------------------------------------------------------------------

/** Load tokenizer + freq dicts + ignore list; streams progress over `onProgress`. */
export async function loadLanguageTools(
	onProgress: (msg: LoadingMessage) => void
): Promise<void> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	await invoke('load_language_tools', { progress: channel });
}

export function getPosCatalog(): Promise<PosInfo[]> {
	return invoke('get_pos_catalog');
}

export function getSettings(): Promise<SettingsData> {
	return invoke('get_settings');
}

export function saveSettings(settings: SettingsData): Promise<void> {
	return invoke('save_settings', { settings });
}

/** Native open dialog; resolves to the chosen path or `null`. */
export function openFileDialog(): Promise<string | null> {
	return invoke('open_file_dialog');
}

/** Parse + segment + filter a file; streams progress; returns the minable terms. */
export async function processFile(
	path: string,
	onProgress: (msg: LoadingMessage) => void
): Promise<FileLoadResult> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	return invoke('process_file', { path, progress: channel });
}

/** The currently loaded file, or `null` if none. */
export function getTerms(): Promise<FileLoadResult | null> {
	return invoke('get_terms');
}

/** Recently-opened files (existing paths only), most-recent first. */
export function getRecentFiles(): Promise<RecentFileEntry[]> {
	return invoke('get_recent_files');
}

/** The ignore list's lemma forms, newest first. */
export function getIgnoreList(): Promise<string[]> {
	return invoke('get_ignore_list');
}

/** Add a lemma to the ignore list; returns the re-filtered file, or `null` if none loaded. */
export function addToIgnoreList(lemma: string): Promise<FileLoadResult | null> {
	return invoke('add_to_ignore_list', { lemma });
}

/** Remove a lemma from the ignore list; returns the re-filtered file, or `null` if none loaded. */
export function removeFromIgnoreList(lemma: string): Promise<FileLoadResult | null> {
	return invoke('remove_from_ignore_list', { lemma });
}

/** The persisted ignore-file shape (a `.txt` of terms + an enabled toggle). */
export interface IgnoreFile {
	path: string;
	enabled: boolean;
}

/** A file pill in the modal: the persisted shape plus display-only metadata. */
export interface IgnoreFileView extends IgnoreFile {
	exists: boolean;
	term_count: number;
}

/** Full ignore-list state that hydrates the modal. */
export interface IgnoreListView {
	terms: string[];
	files: IgnoreFileView[];
}

/** Manual terms + file pills (with `exists`/`term_count`) for the modal. */
export function getIgnoreListFull(): Promise<IgnoreListView> {
	return invoke('get_ignore_list_full');
}

/** Native `.txt` open dialog; returns a staged file pill, or `null` if cancelled. */
export function importIgnoreFile(): Promise<IgnoreFileView | null> {
	return invoke('import_ignore_file');
}

/** Re-read a file's `exists`/`term_count` for display (preserve the staged `enabled`). */
export function refreshIgnoreFile(path: string): Promise<IgnoreFileView> {
	return invoke('refresh_ignore_file', { path });
}

/** Persist staged terms + files and re-filter; returns the updated file, or `null` if none loaded. */
export function saveIgnoreList(
	terms: string[],
	files: IgnoreFile[]
): Promise<FileLoadResult | null> {
	return invoke('save_ignore_list', { terms, files });
}

/** The built-in default ignored terms (for "Restore Default"). */
export function getDefaultIgnoredTerms(): Promise<string[]> {
	return invoke('get_default_ignored_terms');
}

/** Native `.txt` save dialog; writes the staged terms, returns the path or `null`. */
export function exportIgnoreList(terms: string[]): Promise<string | null> {
	return invoke('export_ignore_list', { terms });
}

/** Seek the connected player (mpv or asbplayer) to a sentence timestamp (US3/FR-008). */
export function seekTimestamp(seconds: number, label: string): Promise<void> {
	return invoke('seek_timestamp', { seconds, label });
}

export interface DragDropHandlers {
	/** A drag entered the window; `paths` are the files being dragged. */
	onEnter?: (paths: string[]) => void;
	/** Files were dropped on the window. */
	onDrop?: (paths: string[]) => void;
	/** The drag left the window without dropping. */
	onLeave?: () => void;
}

/** Native OS drag-drop onto the window (the `over` event is ignored). */
export function onDragDrop(handlers: DragDropHandlers): Promise<UnlistenFn> {
	return getCurrentWebview().onDragDropEvent((event) => {
		const p = event.payload;
		if (p.type === 'enter') handlers.onEnter?.(p.paths);
		else if (p.type === 'drop') handlers.onDrop?.(p.paths);
		else if (p.type === 'leave') handlers.onLeave?.();
	});
}

// ---------------------------------------------------------------------------
// Event subscriptions (ambient state pushed by the backend)
// ---------------------------------------------------------------------------

export const onLanguageToolsStatus = (cb: (s: LanguageToolsStatus) => void) =>
	listenTo('language-tools-status', cb);
export const onAnkiStatus = (cb: (s: AnkiStatus) => void) => listenTo('anki-status', cb);
export const onPlayerStatus = (cb: (s: PlayerStatus) => void) => listenTo('player-status', cb);
export const onTermsRefreshed = (cb: (r: FileLoadResult) => void) =>
	listenTo('terms-refreshed', cb);
export const onKnowledgeSummary = (cb: (s: KnowledgeSummary) => void) =>
	listenTo('knowledge-summary', cb);
export const onError = (cb: (e: ErrorPayload) => void) => listenTo('error', cb);

function listenTo<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
	return listen<T>(event, (e) => cb(e.payload));
}
