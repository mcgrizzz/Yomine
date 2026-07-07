//! Frequency-dictionary commands (T028/T060, contracts/commands.md "Frequency
//! dictionaries"). List the live per-dictionary weight/enabled state, mutate a
//! single one (mirrors egui's frequency-weights modal + `apply_frequency_settings`),
//! and import new dictionary zips (mirrors egui's File-menu
//! `frequency_utils::load_frequency_dictionaries`).

use std::sync::{
    atomic::Ordering,
    Arc,
    Mutex,
};

use tauri::{
    ipc::Channel,
    AppHandle,
    Emitter,
    State,
};
use tauri_plugin_dialog::DialogExt;
use yomine::{
    core::settings::FrequencyDictionarySetting,
    dictionary::{
        frequency_manager,
        frequency_utils,
    },
    persistence,
};

use super::apply_frequency_weights;
use crate::{
    dto::DictionaryStateDto,
    events::{
        names,
        LoadingMessage,
    },
    state::AppState,
};

/// The live per-dictionary `{ name, weight, enabled }` set, sorted by name for a
/// stable list. Empty until tools are loaded.
#[tauri::command]
pub fn list_dictionaries(state: State<'_, Mutex<AppState>>) -> Vec<DictionaryStateDto> {
    let guard = state.lock().unwrap();
    let Some(states) =
        guard.language_tools.as_ref().and_then(|t| t.frequency_manager.dictionary_states())
    else {
        return Vec::new();
    };
    let mut dicts: Vec<DictionaryStateDto> = states
        .into_iter()
        .map(|(name, s)| DictionaryStateDto { name, weight: s.weight, enabled: s.enabled })
        .collect();
    dicts.sort_by(|a, b| a.name.cmp(&b.name));
    dicts
}

/// Update one dictionary's weight/enabled, persist it to `settings.frequency_weights`,
/// and emit `dictionaries-changed` so the UI re-fetches terms (weighted frequency /
/// bands changed). Mirrors egui: the manager gets `weight.max(0.1)`, settings keep
/// the raw value; the knowledge summary is marked dirty.
#[tauri::command]
pub fn set_dictionary_state(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    name: String,
    weight: f32,
    enabled: bool,
) -> Result<(), String> {
    let (manager, settings_to_save) = {
        let mut guard = state.lock().unwrap();
        let manager = guard.language_tools.as_ref().map(|t| Arc::clone(&t.frequency_manager));
        guard
            .settings
            .frequency_weights
            .insert(name.clone(), FrequencyDictionarySetting { weight, enabled });
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
        (manager, guard.settings.clone())
    };

    persistence::save_json(&settings_to_save, "settings.json").map_err(|e| e.to_string())?;

    if let Some(manager) = manager {
        manager.set_dictionary_state(&name, weight.max(0.1), enabled).map_err(|e| e.to_string())?;

        // The stored terms carry a baked "HARMONIC" entry computed at process time
        // (egui instead recomputes `get_weighted_harmonic` every frame at render).
        // Refresh it under the new weights so the `get_terms` re-fetch the
        // `dictionaries-changed` event triggers returns up-to-date values.
        let mut guard = state.lock().unwrap();
        let file = &mut guard.file;
        for term in file.terms.iter_mut().chain(file.base_terms.iter_mut()) {
            let harmonic = manager.get_weighted_harmonic(&term.frequencies);
            term.frequencies.insert("HARMONIC".to_string(), harmonic);
        }
    }

    let _ = app.emit(names::DICTIONARIES_CHANGED, ());
    Ok(())
}

/// File → Load New Frequency Dictionaries (T060; egui
/// `frequency_utils::load_frequency_dictionaries`): open a native multi-`.zip`
/// picker, copy the new archives into the frequency-dict dir (already-present
/// filenames are skipped, as in egui), and — only when something new landed —
/// re-run `process_frequency_dictionaries` off the runtime, streaming its
/// progress over `progress`. On success the persisted weights are applied to the
/// new manager, it's swapped into the live tools, the loaded file's per-term
/// frequencies are re-baked against it, and `dictionaries-changed` fires (the UI
/// re-fetches terms + setup status). Returns the number of newly copied archives
/// (0 = cancelled or nothing new → no reload, egui parity).
#[tauri::command]
pub async fn load_frequency_dictionaries(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    progress: Channel<LoadingMessage>,
) -> Result<usize, String> {
    // The reload swaps the manager inside the live tools, so they must exist
    // (the frontend gates its entry points on tools-ready too).
    if state.lock().unwrap().language_tools.is_none() {
        return Err("Language tools are still loading".to_string());
    }

    // Native multi-file picker (egui's `select_frequency_dictionary_zips`).
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Yomitan Frequency Dictionaries", &["zip"])
        .add_filter("All Files", &["*"])
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    let chosen = rx.await.map_err(|_| "file dialog closed unexpectedly".to_string())?;
    let zip_paths: Vec<std::path::PathBuf> =
        chosen.unwrap_or_default().into_iter().filter_map(|p| p.into_path().ok()).collect();
    if zip_paths.is_empty() {
        return Ok(0); // Dialog cancelled.
    }

    let copied =
        frequency_utils::copy_frequency_dictionaries(zip_paths).map_err(|e| e.to_string())?;
    if copied == 0 {
        return Ok(0); // Everything already existed — egui skips the reload too.
    }

    reload_and_swap(&app, &state, &progress).await?;
    Ok(copied)
}

/// Rebuild the frequency manager from the dict dir and swap it into the live
/// tools — the shared tail of every dictionary mutation (T060 import, T064
/// install/update/remove). Re-processes the whole dir off the runtime (extracts
/// new zips; unchanged dicts load from cache) streaming progress; then, following
/// egui's `FrequencyDictionariesReloaded` arm: applies persisted weights, swaps
/// the manager, and marks the knowledge summary dirty. Also re-bakes the loaded
/// terms' full frequency maps against the new manager (`build_freq_map`, the same
/// call `extract_words` bakes with) — a deliberate improvement over egui, where
/// new per-term dictionary entries only appear after the file is reopened
/// (maintainer decision, 2026-07-06). Per-term lookups are HashMap probes, so
/// this is fast even for large files. Emits `dictionaries-changed` on success.
pub(crate) async fn reload_and_swap(
    app: &AppHandle,
    state: &State<'_, Mutex<AppState>>,
    progress: &Channel<LoadingMessage>,
) -> Result<(), String> {
    let progress_for_blocking = progress.clone();
    let reloaded = tauri::async_runtime::spawn_blocking(move || {
        let _ = progress_for_blocking
            .send(LoadingMessage::new("Reloading frequency dictionaries..."));
        let callback = Box::new(move |message: String| {
            let _ = progress_for_blocking.send(LoadingMessage::new(message));
        });
        frequency_manager::process_frequency_dictionaries(Some(callback))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = progress.send(LoadingMessage::clear());
    let manager = Arc::new(reloaded?);

    {
        let mut guard = state.lock().unwrap();
        apply_frequency_weights(&manager, &guard.settings.frequency_weights);
        let Some(tools) = guard.language_tools.as_mut() else {
            return Err("Language tools are still loading".to_string());
        };
        tools.frequency_manager = Arc::clone(&manager);
        let file = &mut guard.file;
        for term in file.terms.iter_mut().chain(file.base_terms.iter_mut()) {
            term.frequencies =
                manager.build_freq_map(&term.lemma_form, &term.lemma_reading, term.is_kana);
        }
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
    }

    let _ = app.emit(names::DICTIONARIES_CHANGED, ());
    Ok(())
}
