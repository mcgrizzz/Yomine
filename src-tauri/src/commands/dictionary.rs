//! Frequency-dictionary commands (T028, contracts/commands.md "Frequency
//! dictionaries"). List the live per-dictionary weight/enabled state and mutate a
//! single one — mirrors egui's frequency-weights modal + `apply_frequency_settings`.

use std::sync::{
    atomic::Ordering,
    Arc,
    Mutex,
};

use tauri::{
    AppHandle,
    Emitter,
    State,
};
use yomine::{
    core::settings::FrequencyDictionarySetting,
    persistence,
};

use crate::{
    dto::DictionaryStateDto,
    events::names,
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
    }

    let _ = app.emit(names::DICTIONARIES_CHANGED, ());
    Ok(())
}
