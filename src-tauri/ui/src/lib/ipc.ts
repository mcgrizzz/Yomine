// Typed IPC layer: thin wrappers over Tauri `invoke` / `listen` / `Channel`,
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
	/** Covering term's Anki state for underline coloring (issue #94);
	 * `null` when no extracted term covers the segment. */
	knowledge: SegmentKnowledge | null;
}

/** Mirrors `SegmentKnowledge` (dto.rs): worst state over overlapping terms. */
export type SegmentKnowledge = 'unknown' | 'new' | 'young' | 'mature';

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
	/** Whether Anki filtering removed any terms — gates the per-sentence
	 * comprehension indicator (egui's `anki_filtered_terms.is_empty()` check). */
	anki_filter_active: boolean;
	/** Total terms before filtering, for the "shown / known / total" summary. */
	total_terms: number;
	/** Terms hidden by the ignore list — the known-count hover breakdown. */
	ignored_terms: number;
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

// ---- Frequency analyzer -----------------------------------------

/** Per-file analysis progress streamed over a `Channel` while `start_analysis`
 * runs (data-model.md). `current_file` is 1-based; `eta_secs` is the smoothed
 * remaining-time estimate (`null` until the first byte lands). */
export interface AnalysisProgressDto {
	total_files: number;
	current_file: number;
	message: string;
	total_bytes: number;
	bytes_processed: number;
	eta_secs: number | null;
}

/** One row of the frequency-analysis preview (`term`/`reading` are the lemma +
 * its reading, `null` reading for pure kana; `frequency`/`count` are the corpus
 * count). */
export interface AnalysisPreviewEntry {
	term: string;
	reading: string | null;
	frequency: number;
	count: number;
}

/** The results preview returned by `start_analysis`: top `PREVIEW_LIMIT` entries
 * by frequency, plus the full unique-lemma `total` before the cap. */
export interface AnalysisPreview {
	entries: AnalysisPreviewEntry[];
	/** The lowest-frequency slice (last ≤PREVIEW_LIMIT of the same desc list),
	 * for the Top 250 / Bottom 250 radio. */
	bottom: AnalysisPreviewEntry[];
	total: number;
}

/** Export metadata + format flags (mirrors `tools::analysis::ExportOptions`).
 * Empty metadata strings map to `None` backend-side. */
export interface ExportOptions {
	dict_name: string;
	dict_author: string;
	dict_url: string;
	dict_description: string;
	revision_prefix: string;
	export_yomitan: boolean;
	export_csv: boolean;
	pretty_json: boolean;
	exclude_hapax: boolean;
}

/** `ExportOptions::default()` — Yomitan on, everything else off/empty. */
export function defaultExportOptions(): ExportOptions {
	return {
		dict_name: 'custom_frequency',
		dict_author: '',
		dict_url: '',
		dict_description: '',
		revision_prefix: '',
		export_yomitan: true,
		export_csv: false,
		pretty_json: false,
		exclude_hapax: false
	};
}

export interface FieldMapping {
	term_field: string;
	reading_field: string;
	/** Sentence field for already-mined detection (issue #3); optional. */
	sentence_field?: string | null;
}

/** A note type with its fields (`core::settings::AnkiModelInfo`). `sample_note`
 * is `null` from `list_anki_models`; it is fetched lazily per model. */
export interface AnkiModelInfo {
	name: string;
	fields: string[];
	sample_note: Record<string, string> | null;
}

/** A model's sample note + the engine's term/reading/sentence field guesses. */
export interface SampleNote {
	sample_note: Record<string, string> | null;
	guessed_term: string | null;
	guessed_reading: string | null;
	guessed_sentence: string | null;
}

export interface FrequencyDictionarySetting {
	weight: number;
	enabled: boolean;
}

/** Mirrors `SentenceColoring` (core/settings.rs, serde lowercase). */
export type SentenceColoring = 'knowledge' | 'none';

/** Mirrors `UnderlineToggles` (core/settings.rs): per-state underline visibility. */
export type UnderlineToggles = Record<SegmentKnowledge, boolean>;

export interface SettingsData {
	anki_model_mappings: Record<string, FieldMapping>;
	anki_interval: number;
	websocket_settings: { port: number };
	frequency_weights: Record<string, FrequencyDictionarySetting>;
	pos_filters: Record<string, boolean>;
	use_serif_font: boolean;
	dark_mode: boolean;
	/** Follow mode (issue #105): after a load from asbplayer, NEW bound videos
	 * with subtitles load automatically. Opt-in. */
	asbplayer_follow_new_media: boolean;
	/** Follow mode (issue #105): switch to the active tab's video when it isn't
	 * the loaded one. Opt-in. */
	asbplayer_follow_active_tab: boolean;
	/** Follow-mode poll cadence in seconds (≥1). */
	asbplayer_poll_secs: number;
	/** Whole-UI scale factor (1.0 = 100%), applied as CSS zoom on the root. */
	font_scale: number;
	/** yomitan-api base URL (one-click mining, issue #105). */
	yomitan_url: string;
	/** How sentence segments are colored in the term table (issue #94). */
	sentence_coloring: SentenceColoring;
	/** Which underline states are shown in knowledge mode. */
	sentence_underlines: UnderlineToggles;
}

/** Aggregated setup readiness for the checklist/banner (`get_setup_status`).
 * Each field mirrors a `check_*` in egui's `setup_checklist_modal.rs`. */
export interface SetupStatus {
	tools_loaded: boolean;
	anki_connected: boolean;
	has_field_mapping: boolean;
	has_frequency_dict: boolean;
	/** Loaded dictionary count: ≥1 → item 2 (default) complete, >1 → item 6 (additional). */
	frequency_dict_count: number;
	player_connected: boolean;
	/** yomitan-api reachable (optional item — enables one-click mining). */
	yomitan_connected: boolean;
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
	/** WebSocket server state — drives the asbplayer dot's sub-states. */
	server_state: 'running' | 'starting' | 'error' | 'stopped';
	/** Error message when `server_state === 'error'`, else null. */
	server_error: string | null;
	/** Start-seconds the player acknowledged seeking to this session —
	 * timestamp buttons matching an entry show egui's 👁 confirmed state. */
	confirmed_timestamps: number[];
}

export interface ErrorPayload {
	title: string;
	message: string;
	detail: string | null;
}

/** `export-complete` payload. */
export interface ExportCompletePayload {
	ok: boolean;
	message: string;
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

/** Reapply ignore + live Anki filters (egui's 🔄 / F5 / Cmd+R). The updated
 * file arrives via the `terms-refreshed` event; no-op when nothing is loaded. */
export function refreshTerms(): Promise<void> {
	return invoke('refresh_terms');
}

/** Recently-opened files (existing paths only), most-recent first. */
export function getRecentFiles(): Promise<RecentFileEntry[]> {
	return invoke('get_recent_files');
}

/** The ignore list's lemma forms, newest first. */
export function getIgnoreList(): Promise<string[]> {
	return invoke('get_ignore_list');
}

/** Add a lemma to the ignore list (persists). Does not re-filter — the term stays
 * greyed-in-place until the next refresh. */
export function addToIgnoreList(lemma: string): Promise<void> {
	return invoke('add_to_ignore_list', { lemma });
}

/** Remove a lemma from the ignore list (persists). Does not re-filter; the term just
 * stops being greyed. */
export function removeFromIgnoreList(lemma: string): Promise<void> {
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

/** Open the app data directory in the OS file explorer (File → Open Data Folder). */
export function openDataFolder(): Promise<void> {
	return invoke('open_data_folder');
}

/** Seek the connected player (mpv or asbplayer) to a sentence timestamp. */
export function seekTimestamp(seconds: number, label: string): Promise<void> {
	return invoke('seek_timestamp', { seconds, label });
}

/** One subtitle track loaded for a bound media (issue #105). */
export interface SubtitleTrack {
	track_number: number;
	file_name: string;
}

/** Media asbplayer is currently tracking (`get-bound-media`, issue #105). */
export interface BoundMedia {
	id: string;
	/** 'streaming' | 'local'. */
	media_type: string;
	title: string | null;
	favicon_url: string | null;
	/** Empty when no subtitles are loaded for the media. */
	loaded_subtitles: SubtitleTrack[];
	/** Whether the media's tab is the active tab of its window. */
	active: boolean;
}

/** The media asbplayer is tracking, for the "Load from asbplayer" picker
 * (issue #105). Rejects when asbplayer isn't connected or the extension
 * predates `get-bound-media` (v1.20+). */
export function getAsbplayerMedia(): Promise<BoundMedia[]> {
	return invoke('get_asbplayer_media');
}

/** Fetch a media's subtitles from asbplayer and run them through the same
 * pipeline as a file; resolves with the loaded terms (issue #105). Timestamps
 * are preserved, so seeking works. `trackNumbers = null` loads all tracks. */
export async function loadAsbplayerMedia(
	mediaId: string,
	trackNumbers: number[] | null,
	title: string,
	onProgress: (msg: LoadingMessage) => void
): Promise<FileLoadResult> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	return invoke('load_asbplayer_media', { mediaId, trackNumbers, title, progress: channel });
}

/** `mine_term` outcome; `warning` = note created but enrichment failed. */
export interface MineResult {
	status: 'created' | 'duplicate';
	via: string;
	warning: string | null;
	note_id: number | null;
}

/** Already-mined state (issue #3); sentences are `normalizeSentence` keys. */
export interface MinedState {
	added_terms: string[];
	mined_sentences: string[];
}

export interface YomitanStatus {
	reachable: boolean;
	version: string | null;
}

/** One-click mine (issue #105); stage updates stream through `onProgress`. */
export function mineTerm(
	args: {
		term: string;
		sentence: string;
		timestampSecs: number | null;
		timestampLabel: string | null;
		via: 'asbplayer' | 'direct';
	},
	onProgress: (msg: LoadingMessage) => void
): Promise<MineResult> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	return invoke('mine_term', { ...args, progress: channel });
}

/** Open Anki's card browser on a mined note (`guiBrowse nid:`). */
export function openInAnki(noteId: number): Promise<void> {
	return invoke('open_in_anki', { noteId });
}

/** Best-effort: an offline AnkiConnect still returns cached sentences. */
export function getMinedState(): Promise<MinedState> {
	return invoke('get_mined_state');
}

/** Reachability probe; `url` tests a staged value (omitted = saved setting). */
export function getYomitanStatus(url?: string): Promise<YomitanStatus> {
	return invoke('get_yomitan_status', { url: url ?? null });
}

/** Persist the WebSocket server port and restart a running server on it. */
export function setWebsocketPort(port: number): Promise<void> {
	return invoke('set_websocket_port', { port });
}

/** Snapshot of the player status. The `player-status` event only fires on *change*,
 * so a freshly-loaded webview must pull the resting state once (hydrate). */
export function getPlayerStatus(): Promise<PlayerStatus> {
	return invoke('get_player_status');
}

/** Snapshot of Anki connectivity (same hydrate rationale as getPlayerStatus). */
export function getAnkiStatus(): Promise<AnkiStatus> {
	return invoke('get_anki_status');
}

/** `null` until the background task has produced one. One-shot hydrate so a
 * (re)loaded webview isn't blank — the event only fires on change. */
export function getKnowledgeSummary(): Promise<KnowledgeSummary | null> {
	return invoke('get_knowledge_summary');
}

/** Note types (with fields) that have at least one note, for the Anki settings
 * modal's mapping UI. Rejects with "Anki Offline" when disconnected. */
export function listAnkiModels(): Promise<AnkiModelInfo[]> {
	return invoke('list_anki_models');
}

/** Fetch a model's sample note + the engine-side field guesses.
 * Never rejects — fetch failures come back as a `null` sample (egui parity). */
export function getAnkiSampleNote(modelName: string, fields: string[]): Promise<SampleNote> {
	return invoke('get_anki_sample_note', { modelName, fields });
}

/** One row of the frequency-dictionary list (`DictionaryStateDto`). */
export interface DictionaryState {
	name: string;
	weight: number;
	enabled: boolean;
}

/** The live per-dictionary weight/enabled set, sorted by name.
 * Empty until the language tools are loaded. */
export function listDictionaries(): Promise<DictionaryState[]> {
	return invoke('list_dictionaries');
}

/** Update one dictionary's weight/enabled: persists `settings.frequency_weights`,
 * applies to the live manager, rebakes the stored terms' HARMONIC, and emits
 * `dictionaries-changed`. */
export function setDictionaryState(name: string, weight: number, enabled: boolean): Promise<void> {
	return invoke('set_dictionary_state', { name, weight, enabled });
}

/** One row of the dictionary manager's "Recommended" section (issue #100). */
export interface RecommendedDictionary {
	name: string;
	title: string;
	description: string;
	installed_revision: string | null;
	latest_revision: string | null;
	/** Backend-derived: 'installed' = present but latest revision unknown. */
	status: 'not-installed' | 'installed' | 'up-to-date' | 'update-available';
}

/** The recommended catalog with install/update state resolved. Fetches
 * the repo manifest + live update indexes, so it needs network for update
 * badges — offline it falls back to the baked manifest. */
export function getRecommendedDictionaries(): Promise<RecommendedDictionary[]> {
	return invoke('get_recommended_dictionaries');
}

/** Install or update a recommended dictionary by title; replaces existing
 * artifacts of the same title, reloads + re-bakes, emits `dictionaries-changed`.
 * Progress streams over `onProgress`. */
export async function installRecommendedDictionary(
	title: string,
	onProgress: (msg: LoadingMessage) => void
): Promise<void> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	return invoke('install_recommended_dictionary', { title, progress: channel });
}

/** Remove an installed dictionary (any, not just recommended): deletes its files
 * + persisted weight, reloads, emits `dictionaries-changed`. */
export async function removeDictionary(
	title: string,
	onProgress: (msg: LoadingMessage) => void
): Promise<void> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	return invoke('remove_dictionary', { title, progress: channel });
}

/** Zip import via native multi-`.zip` picker. Resolves with the number of newly
 * copied archives — 0 means cancelled or nothing new, i.e. no reload happened. */
export async function loadFrequencyDictionaries(
	onProgress: (msg: LoadingMessage) => void
): Promise<number> {
	const channel = new Channel<LoadingMessage>();
	channel.onmessage = onProgress;
	return invoke('load_frequency_dictionaries', { progress: channel });
}

/** Aggregated setup readiness. Probes Anki + player live, so it's a
 * command (not an event) — pull on hydrate and after relevant state changes. */
export function getSetupStatus(): Promise<SetupStatus> {
	return invoke('get_setup_status');
}

export interface UpdateInfo {
	current: string;
	/** Latest release tag, e.g. "v0.6.1". */
	latest: string;
	url: string;
}

/** `null` = up to date. Checks the newest (non-prerelease) GitHub release. */
export function checkForUpdate(): Promise<UpdateInfo | null> {
	return invoke('check_for_update');
}

/** Expand a picked folder to the supported subtitle/text files under it
 * (recurses subdirectories) — used to build the selection tree. */
export function findAnalysisFiles(dir: string): Promise<string[]> {
	return invoke('find_analysis_files', { dir });
}

/** Tokenize the corpus + count lemma frequencies; streams `AnalysisProgressDto`
 * over `onProgress` and resolves with the top-`PREVIEW_LIMIT` preview. Rejects
 * with "...cancelled..." on user cancel, or an error message otherwise. */
export async function startAnalysis(
	paths: string[],
	balanceCorpus: boolean,
	onProgress: (p: AnalysisProgressDto) => void
): Promise<AnalysisPreview> {
	const channel = new Channel<AnalysisProgressDto>();
	channel.onmessage = onProgress;
	return invoke('start_analysis', { paths, balanceCorpus, progress: channel });
}

/** Request cancellation of a running `start_analysis`. */
export function cancelAnalysis(): Promise<void> {
	return invoke('cancel_analysis');
}

/** Export the last analysis into `output_dir` per the `options` flags; resolves
 * with a success message. */
export function exportAnalysis(output_dir: string, options: ExportOptions): Promise<string> {
	return invoke('export_analysis', { outputDir: output_dir, options });
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
export const onDictionariesChanged = (cb: () => void) =>
	listenTo<null>('dictionaries-changed', () => cb());
/** Follow mode auto-loaded a new asbplayer video (issue #105); payload = the
 * freshly loaded file. */
export const onAsbplayerMediaLoaded = (cb: (r: FileLoadResult) => void) =>
	listenTo('asbplayer-media-loaded', cb);
export const onError = (cb: (e: ErrorPayload) => void) => listenTo('error', cb);

// The analyzer commands already resolve with their result; the modal must NOT
// also wire these events or it double-handles the same outcome.
export const onAnalysisComplete = (cb: (p: AnalysisPreview) => void) =>
	listenTo('analysis-complete', cb);
export const onAnalysisCancelled = (cb: () => void) =>
	listenTo<null>('analysis-cancelled', () => cb());
export const onExportComplete = (cb: (p: ExportCompletePayload) => void) =>
	listenTo('export-complete', cb);

function listenTo<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
	return listen<T>(event, (e) => cb(e.payload));
}
