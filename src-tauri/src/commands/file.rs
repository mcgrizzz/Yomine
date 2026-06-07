//! File / mining commands (T021, contracts/commands.md "File / mining").

use std::sync::Mutex;

use tauri::{
    ipc::Channel,
    AppHandle,
    State,
};
use tauri_plugin_dialog::DialogExt;
use yomine::{
    core::{
        filename_parser,
        models::{
            SourceFile,
            SourceFileType,
        },
        pipeline::process_source_file,
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
    events::LoadingMessage,
    state::{
        AppState,
        FileData,
    },
};

/// egui uses id 3 for ad-hoc opened files; match it so downstream ids align.
const DEFAULT_SOURCE_FILE_ID: u32 = 3;

/// Build a `FileLoadResult` from the stored file state (sentences → DTOs).
fn load_result(file: &FileData) -> Option<FileLoadResult> {
    let source_file = file.source_file.clone()?;
    Some(FileLoadResult {
        source_file,
        terms: file.terms.clone(),
        sentences: file.sentences.iter().map(SentenceDto::from_sentence).collect(),
        file_comprehension: file.file_comprehension,
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

    let mut guard = state.lock().unwrap();
    guard.file = FileData {
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        sentences,
        file_comprehension,
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    let _ = progress.send(LoadingMessage::clear());
    Ok(payload)
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
