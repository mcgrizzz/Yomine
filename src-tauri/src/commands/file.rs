//! File / mining commands (T021, contracts/commands.md "File / mining").

use std::sync::{
    atomic::Ordering,
    Mutex,
};

use tauri::{
    ipc::Channel,
    AppHandle,
    Emitter,
    Manager,
    State,
};
use tauri_plugin_dialog::DialogExt;
use yomine::{
    anki::comprehensibility::calculate_sentence_comprehension,
    core::{
        filename_parser,
        models::{
            SourceFile,
            SourceFileType,
        },
        pipeline::{
            apply_filters,
            process_source_file,
            AnkiFilter,
        },
        recent_files::{
            RecentFileEntry,
            RecentFiles,
        },
    },
    persistence::{
        load_json_or_default,
        save_json,
    },
};

use crate::{
    dto::{
        FileLoadResult,
        SentenceDto,
    },
    events::{
        names,
        AnkiStatus,
        ErrorPayload,
        LoadingMessage,
    },
    state::{
        AppState,
        FileData,
    },
};

/// egui uses id 3 for ad-hoc opened files; match it so downstream ids align.
const DEFAULT_SOURCE_FILE_ID: u32 = 3;

/// Build a `FileLoadResult` from the stored file state (sentences → DTOs).
pub(crate) fn load_result(file: &FileData) -> Option<FileLoadResult> {
    let source_file = file.source_file.clone()?;
    Some(FileLoadResult {
        source_file,
        terms: file.terms.clone(),
        sentences: file.sentences.iter().map(SentenceDto::from_sentence).collect(),
        file_comprehension: file.file_comprehension,
        anki_filter_active: !file.anki_known_lemmas.is_empty(),
        total_terms: file.base_terms.len(),
        ignored_terms: file.ignored_count,
    })
}

/// Construct a `SourceFile` from a filesystem path, parsing title/creator from the
/// filename the same way the egui file modal does.
fn source_file_from_path(path: &str) -> SourceFile {
    let filename =
        std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
    let media_info = filename_parser::parse_filename(filename);
    let metadata = media_info.get_metadata_string();
    SourceFile {
        id: DEFAULT_SOURCE_FILE_ID,
        source: None,
        file_type: SourceFileType::from_extension(path),
        title: media_info.display_title(),
        creator: if metadata.is_empty() { None } else { Some(metadata) },
        original_file: path.to_string(),
    }
}

/// Native open dialog (FR: file selection). Returns the chosen path or `null`.
#[tauri::command]
pub async fn open_file_dialog(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Subtitles & text", SourceFileType::supported_extensions())
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let chosen = rx.await.map_err(|_| "file dialog closed unexpectedly".to_string())?;
    Ok(chosen
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string()))
}

/// Parse + segment + filter a source file (cached Anki snapshot, offline-safe) and
/// return the minable terms + sentence DTOs. Stores the result in `AppState`.
#[tauri::command]
pub async fn process_file(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    path: String,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    let tools = state
        .lock()
        .unwrap()
        .language_tools
        .clone()
        .ok_or_else(|| "Language tools are still loading".to_string())?;

    let _ = progress.send(LoadingMessage::new("Processing file..."));
    let source_file = source_file_from_path(&path);

    // The backend runs in its own process (the UI is a separate webview), so a
    // brief parse/segment on the async runtime doesn't freeze the UI. The cached
    // Anki path does no real I/O await; the heavy work is CPU segmentation.
    let (base_terms, filter_result, sentences, file_comprehension) =
        process_source_file(&source_file, &tools).await.map_err(|e| e.to_string())?;

    // Record the file in the shared `recent_files.json` (same store as egui) so it
    // appears on the landing state, mirroring egui's `add_recent_file`.
    record_recent_file(&source_file, filter_result.terms.len());

    // Lemmas Anki already knew — kept so an ignore-list change can re-filter
    // without re-querying Anki (mirrors egui's `anki_filtered_terms`).
    let anki_known_lemmas =
        filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();

    let mut guard = state.lock().unwrap();
    guard.file = FileData {
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        anki_known_lemmas,
        ignored_count: filter_result.ignore_filtered.len(),
        sentences,
        file_comprehension,
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    // The table now shows the cached Anki snapshot; if Anki is live, refresh
    // against it in the background and update in place via `terms-refreshed`
    // (egui's `handle_processing_result` tail → `refresh_terms`).
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if yomine::anki::api::get_version().await.is_ok() {
            if let Err(e) = live_refresh(&app_handle).await {
                let _ = app_handle.emit(
                    names::ERROR,
                    ErrorPayload {
                        title: "Refresh Error".into(),
                        message: "Unable to refresh terms".into(),
                        detail: Some(e),
                    },
                );
            }
        }
    });

    let _ = progress.send(LoadingMessage::clear());
    Ok(payload)
}

/// Re-partition the loaded file's terms against **live** Anki data and emit
/// `terms-refreshed` — a port of egui's `TaskManager::refresh_terms` + the
/// `TermsRefreshed` handler: re-applies ignore + live Anki filters, recomputes
/// per-sentence and file comprehension, stores the result, and marks the
/// knowledge summary dirty (the live fetch just rewrote the vocab cache).
pub(crate) async fn live_refresh(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let (tools, base_terms, mut sentences, mappings) = {
        let guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        // Nothing loaded → nothing to refresh (egui's RequestRefresh no-ops too).
        if guard.file.base_terms.is_empty() {
            return Ok(());
        }
        (
            tools,
            guard.file.base_terms.clone(),
            guard.file.sentences.clone(),
            guard.settings.anki_model_mappings.clone(),
        )
    };

    // Mirror egui's `anki_fetching = true` spinner while the live fetch runs.
    let _ = app.emit(names::ANKI_STATUS, AnkiStatus { connected: true, fetching: true });

    let outcome: Result<FileLoadResult, String> = async {
        let filter_result = apply_filters(base_terms, &tools, AnkiFilter::Live(mappings))
            .await
            .map_err(|e| e.to_string())?;

        // Reconstruct the full term set and recompute comprehension from it
        // (egui `refresh_terms` does the same before sending `TermsRefreshed`).
        let mut all_terms = Vec::new();
        all_terms.extend(filter_result.terms.iter().cloned());
        all_terms.extend(filter_result.anki_filtered.iter().cloned());
        all_terms.extend(filter_result.ignore_filtered.iter().cloned());
        for sentence in &mut sentences {
            calculate_sentence_comprehension(sentence, &all_terms);
        }
        let file_comprehension = if sentences.is_empty() {
            0.0
        } else {
            sentences.iter().map(|s| s.comprehension).sum::<f32>() / sentences.len() as f32
        };

        let mut guard = state.lock().unwrap();
        guard.file.anki_known_lemmas =
            filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
        guard.file.ignored_count = filter_result.ignore_filtered.len();
        guard.file.terms = filter_result.terms;
        guard.file.base_terms = all_terms;
        guard.file.sentences = sentences;
        guard.file.file_comprehension = file_comprehension;
        // Recompute coverage from the fresh vocab cache (egui resets
        // `knowledge_summary_attempted`).
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
        Ok(load_result(&guard.file).expect("refreshed file has a source_file"))
    }
    .await;

    let _ = app.emit(
        names::ANKI_STATUS,
        AnkiStatus { connected: outcome.is_ok(), fetching: false },
    );
    let payload = outcome?;
    let _ = app.emit(names::TERMS_REFRESHED, &payload);
    Ok(())
}

/// Manual "reapply ignorelist and Anki filters" (egui's top-bar 🔄 / F5 / Cmd+R
/// → `RequestRefresh`). The updated file arrives via the `terms-refreshed` event.
#[tauri::command]
pub async fn refresh_terms(app: AppHandle) -> Result<(), String> {
    live_refresh(&app).await
}

/// Re-fetch the currently loaded file (e.g. after a UI reload). `null` if none.
#[tauri::command]
pub fn get_terms(state: State<'_, Mutex<AppState>>) -> Option<FileLoadResult> {
    load_result(&state.lock().unwrap().file)
}

/// Add (or refresh) a file in the shared recent-files store. Failures are logged,
/// not surfaced — a recent-files write must never fail an otherwise-good load.
fn record_recent_file(source_file: &SourceFile, term_count: usize) {
    let mut recent = load_json_or_default::<RecentFiles>("recent_files.json");
    recent.add_file(
        source_file.original_file.clone(),
        source_file.title.clone(),
        source_file.creator.clone(),
        term_count,
    );
    if let Err(e) = save_json(&recent, "recent_files.json") {
        eprintln!("Failed to save recent files: {e}");
    }
}

/// Recent files for the landing state (FR-001), most-recent first. Only entries
/// whose path still exists are returned (egui's `get_valid_files`).
#[tauri::command]
pub fn get_recent_files() -> Vec<RecentFileEntry> {
    let recent = load_json_or_default::<RecentFiles>("recent_files.json");
    recent.get_valid_files().into_iter().cloned().collect()
}
