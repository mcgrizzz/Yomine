//! Frequency-dictionary commands (contracts/commands.md "Frequency dictionaries").

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

/// Update one dictionary's weight/enabled and persist it. The manager gets
/// `weight.max(0.1)`; settings keep the raw value (egui parity).
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

        // The stored terms carry a HARMONIC entry baked at process time; rebake
        // it so the event-triggered `get_terms` re-fetch sees the new weights.
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

/// Zip import via native multi-`.zip` picker. Returns the number of newly
/// copied archives — 0 (cancelled, or every filename already present) skips
/// the reload entirely.
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
/// tools — the shared tail of every dictionary mutation. Also re-bakes the
/// loaded terms' frequency maps against the new manager (a deliberate
/// improvement over egui, where new entries only appeared after reopening the
/// file); per-term lookups are HashMap probes, so this is cheap.
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
