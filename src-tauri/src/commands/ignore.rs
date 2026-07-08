//! Ignore-list commands (contracts/commands.md "Ignore list"). Row-level
//! `add`/`remove` return WITHOUT re-filtering — a just-ignored term stays
//! visible-but-greyed until the next `refresh_terms` (egui parity). Only the
//! modal's `save_ignore_list` re-applies filters.

use std::sync::Mutex;

use tauri::{
    AppHandle,
    State,
};
use tauri_plugin_dialog::DialogExt;
use yomine::core::{
    ignore_list::IgnoreFile,
    pipeline::{
        apply_filters,
        AnkiFilter,
    },
    IgnoreList,
    DEFAULT_IGNORED_TERMS,
};

use crate::{
    commands::file::load_result,
    dto::{
        FileLoadResult,
        IgnoreFileView,
        IgnoreListView,
    },
    state::AppState,
};

/// Add a lemma to the ignore list (persists). Does **not** re-filter — the term
/// stays visible-but-greyed until the next `refresh_terms` (egui parity).
#[tauri::command]
pub async fn add_to_ignore_list(
    state: State<'_, Mutex<AppState>>,
    lemma: String,
) -> Result<(), String> {
    mutate_ignore_list(&state, &lemma, true).await
}

/// Remove a lemma from the ignore list (persists). Does **not** re-filter; the
/// un-ignored term simply stops being greyed (egui parity).
#[tauri::command]
pub async fn remove_from_ignore_list(
    state: State<'_, Mutex<AppState>>,
    lemma: String,
) -> Result<(), String> {
    mutate_ignore_list(&state, &lemma, false).await
}

/// The ignore list's lemma forms, newest first (egui's `get_all_terms`).
#[tauri::command]
pub fn get_ignore_list(state: State<'_, Mutex<AppState>>) -> Result<Vec<String>, String> {
    let guard = state.lock().unwrap();
    let tools = guard
        .language_tools
        .as_ref()
        .ok_or_else(|| "Language tools are still loading".to_string())?;
    let list = tools.ignore_list.lock().map_err(|_| "Failed to lock ignore list".to_string())?;
    Ok(list.get_all_terms())
}

/// Shared row-level add/remove path. Mutates + persists only; no re-filter.
async fn mutate_ignore_list(
    state: &State<'_, Mutex<AppState>>,
    lemma: &str,
    add: bool,
) -> Result<(), String> {
    let tools = {
        let guard = state.lock().unwrap();
        guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?
    };

    let mut list =
        tools.ignore_list.lock().map_err(|_| "Failed to lock ignore list".to_string())?;
    if add { list.add_term(lemma) } else { list.remove_term(lemma) }
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Build a display pill for `path`: `exists` + the file's `term_count` (0 when the
/// file is missing/unreadable, matching egui's count map which only inserts on a
/// successful read).
fn file_view(path: String, enabled: bool) -> IgnoreFileView {
    let exists = IgnoreList::file_exists(&path);
    let term_count = IgnoreList::load_terms_from_file(&path).map(|t| t.len()).unwrap_or(0);
    IgnoreFileView { path, enabled, exists, term_count }
}

/// Full ignore-list state for the modal: manual terms + file pills with per-file
/// `exists` + `term_count`.
#[tauri::command]
pub fn get_ignore_list_full(state: State<'_, Mutex<AppState>>) -> Result<IgnoreListView, String> {
    let guard = state.lock().unwrap();
    let tools = guard
        .language_tools
        .as_ref()
        .ok_or_else(|| "Language tools are still loading".to_string())?;
    let list = tools.ignore_list.lock().map_err(|_| "Failed to lock ignore list".to_string())?;
    let terms = list.get_all_terms();
    let files = list.get_files().into_iter().map(|f| file_view(f.path, f.enabled)).collect();
    Ok(IgnoreListView { terms, files })
}

/// Open a `.txt` open dialog, load its terms, and return a file pill the frontend
/// stages into its file list (persisted on save). `null` if cancelled. Mirrors
/// egui's `FileAction::Add`.
#[tauri::command]
pub async fn import_ignore_file(app: AppHandle) -> Result<Option<IgnoreFileView>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().add_filter("Text files", &["txt"]).pick_file(move |path| {
        let _ = tx.send(path);
    });
    let chosen = rx.await.map_err(|_| "file dialog closed unexpectedly".to_string())?;
    Ok(chosen.and_then(|p| p.into_path().ok()).map(|p| file_view(p.display().to_string(), true)))
}

/// Re-read a file's `exists` + `term_count` for display (the persisted cache reload
/// happens on save). The frontend preserves the staged `enabled`.
#[tauri::command]
pub fn refresh_ignore_file(path: String) -> IgnoreFileView {
    file_view(path, true)
}

/// The modal's single commit point: replace terms + files, reapply filters,
/// return the updated file (`null` if none loaded).
#[tauri::command]
pub async fn save_ignore_list(
    state: State<'_, Mutex<AppState>>,
    terms: Vec<String>,
    files: Vec<IgnoreFile>,
) -> Result<Option<FileLoadResult>, String> {
    let (tools, base_terms, anki_known) = {
        let guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        (tools, guard.file.base_terms.clone(), guard.file.anki_known_lemmas.clone())
    };

    {
        let mut list =
            tools.ignore_list.lock().map_err(|_| "Failed to lock ignore list".to_string())?;
        list.set_terms(terms).map_err(|e| e.to_string())?;
        // `set_files` persists and calls `reload_file_cache`.
        list.set_files(files).map_err(|e| e.to_string())?;
    }

    // No file loaded → nothing to re-filter, but the list change persisted.
    if base_terms.is_empty() {
        return Ok(None);
    }

    let filter_result = apply_filters(base_terms, &tools, AnkiFilter::KnownLemmas(anki_known))
        .await
        .map_err(|e| e.to_string())?;

    let mut guard = state.lock().unwrap();
    guard.file.terms = filter_result.terms;
    guard.file.ignored_count = filter_result.ignore_filtered.len();
    Ok(load_result(&guard.file))
}

/// The built-in default ignored terms, for the modal's "Restore Default" (staged
/// client-side, persisted on save).
#[tauri::command]
pub fn get_default_ignored_terms() -> Vec<String> {
    DEFAULT_IGNORED_TERMS.iter().map(|s| s.to_string()).collect()
}

/// Open a `.txt` save dialog and write the (possibly unsaved) staged terms
/// newline-joined. Returns the path written, or `null` if cancelled.
#[tauri::command]
pub async fn export_ignore_list(
    app: AppHandle,
    terms: Vec<String>,
) -> Result<Option<String>, String> {
    let default_filename =
        format!("yomine_ignored_terms_{}.txt", chrono::Local::now().format("%Y-%m-%d"));

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Text files", &["txt"])
        .set_file_name(&default_filename)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let chosen = rx.await.map_err(|_| "file dialog closed unexpectedly".to_string())?;

    let Some(path) = chosen.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&path, terms.join("\n")).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(Some(path.display().to_string()))
}
