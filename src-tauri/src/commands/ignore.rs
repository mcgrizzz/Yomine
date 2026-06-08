//! Ignore-list commands (T038, contracts/commands.md "Ignore list").
//!
//! `add`/`remove` mutate the shared `IgnoreList` (persisted to `ignore_list.json`,
//! the same store egui uses) and then re-apply the ignore + cached-Anki filter to
//! the loaded file, mirroring egui's `partial_refresh`: the Anki-known lemmas are
//! passed as `AnkiFilter::KnownLemmas` so known terms stay filtered out and only
//! the ignore set changes.

use std::sync::Mutex;

use tauri::State;
use yomine::core::pipeline::{
    apply_filters,
    AnkiFilter,
};

use crate::{
    commands::file::load_result,
    dto::FileLoadResult,
    state::AppState,
};

/// Add a lemma to the ignore list and return the re-filtered terms. `null` when no
/// file is loaded (the list change still persists).
#[tauri::command]
pub async fn add_to_ignore_list(
    state: State<'_, Mutex<AppState>>,
    lemma: String,
) -> Result<Option<FileLoadResult>, String> {
    mutate_ignore_list(&state, &lemma, true).await
}

/// Remove a lemma from the ignore list and return the re-filtered terms. `null`
/// when no file is loaded.
#[tauri::command]
pub async fn remove_from_ignore_list(
    state: State<'_, Mutex<AppState>>,
    lemma: String,
) -> Result<Option<FileLoadResult>, String> {
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

/// Shared add/remove path: briefly lock state to clone the handles + re-filter
/// inputs, mutate (and persist) the ignore list, then re-apply filters and store
/// the new minable set. The `Mutex<AppState>` is never held across the `.await`.
async fn mutate_ignore_list(
    state: &State<'_, Mutex<AppState>>,
    lemma: &str,
    add: bool,
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
        // `add_term`/`remove_term` persist to disk on change.
        if add { list.add_term(lemma) } else { list.remove_term(lemma) }
            .map_err(|e| e.to_string())?;
    }

    // No file loaded → nothing to re-filter, but the list change persisted.
    if base_terms.is_empty() {
        return Ok(None);
    }

    // Re-apply ignore + cached-Anki filter (no Anki connection); mirrors
    // egui's `partial_refresh`.
    let filter_result = apply_filters(base_terms, &tools, AnkiFilter::KnownLemmas(anki_known))
        .await
        .map_err(|e| e.to_string())?;

    let mut guard = state.lock().unwrap();
    guard.file.terms = filter_result.terms;
    Ok(load_result(&guard.file))
}
