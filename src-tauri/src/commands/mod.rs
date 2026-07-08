//! The Tauri command layer — the stable IPC boundary (contracts/commands.md).
//!
//! Locking discipline (see `state.rs`): commands lock `Mutex<AppState>` only
//! briefly — lock → clone the cheap `Arc` handles → unlock → do async/blocking
//! work → lock again to store. The lock is **never** held across an `.await`.

pub mod analysis;
pub mod anki;
pub mod dictionary;
pub mod file;
pub mod ignore;
pub mod knowledge;
pub mod lifecycle;
pub mod player;
pub mod recommended;
pub mod setup;

use std::collections::HashMap;

use yomine::{
    core::settings::FrequencyDictionarySetting,
    dictionary::frequency_manager::FrequencyManager,
};

/// Push persisted weights/enabled flags into the live frequency manager, mirroring
/// egui's `apply_frequency_settings`. Dictionaries with no saved setting keep the
/// manager's own default; failures are logged per-dict (non-fatal) as in egui.
pub fn apply_frequency_weights(
    manager: &FrequencyManager,
    weights: &HashMap<String, FrequencyDictionarySetting>,
) {
    if let Some(states) = manager.dictionary_states() {
        for (name, state) in states {
            let setting = weights.get(&name).cloned().unwrap_or(FrequencyDictionarySetting {
                weight: state.weight,
                enabled: state.enabled,
            });
            let weight = setting.weight.max(0.1);
            if let Err(err) = manager.set_dictionary_state(&name, weight, setting.enabled) {
                eprintln!("Failed to update dictionary state '{}': {}", name, err);
            }
        }
    }
}
