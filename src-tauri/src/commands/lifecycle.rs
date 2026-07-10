//! Lifecycle / tools commands (contracts/commands.md "Lifecycle / tools").

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
use tauri_plugin_opener::OpenerExt;
use yomine::{
    core::{
        settings::SettingsData,
        IgnoreList,
        LanguageTools,
    },
    dictionary::{
        frequency_manager,
        token_dictionary::DictType,
    },
    jlpt::JlptDatabase,
    persistence,
    segmentation::{
        tokenizer::init_vibrato,
        word::POS,
    },
};

use super::apply_frequency_weights;
use crate::{
    dto::PosInfo,
    events::{
        names,
        LanguageToolsStatus,
        LoadingMessage,
    },
    state::AppState,
};

/// Build a boxed progress callback (the engine loaders take `Fn(String)`) that
/// streams each message to the command's `Channel`.
fn progress_callback(channel: Channel<LoadingMessage>) -> Box<dyn Fn(String) + Send> {
    Box::new(move |message: String| {
        let _ = channel.send(LoadingMessage::new(message));
    })
}

/// Idempotent: a second call after success just re-emits `ready`.
#[tauri::command]
pub async fn load_language_tools(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    progress: Channel<LoadingMessage>,
) -> Result<(), String> {
    // Already loaded? Re-emit `ready` so a reloaded UI rehydrates its status.
    if state.lock().unwrap().language_tools.is_some() {
        let _ = app.emit(names::LANGUAGE_TOOLS_STATUS, LanguageToolsStatus::Ready);
        return Ok(());
    }

    // `known_interval` comes from settings; set on the tools once built (egui parity).
    let known_interval = state.lock().unwrap().settings.anki_interval;

    // The loaders block (dictionary download + parse), so run them off the async
    // runtime; the `Channel` is `Send`/`Sync` and carries progress out.
    let progress_for_blocking = progress.clone();
    let loaded = tauri::async_runtime::spawn_blocking(move || -> Result<LanguageTools, String> {
        let _ = progress_for_blocking.send(LoadingMessage::new("Loading tokenizer..."));
        let tokenizer = Arc::new(
            init_vibrato(&DictType::Unidic, Some(progress_callback(progress_for_blocking.clone())))
                .map_err(|e| e.to_string())?,
        );

        let _ =
            progress_for_blocking.send(LoadingMessage::new("Loading frequency dictionaries..."));
        let frequency_manager = Arc::new(
            frequency_manager::process_frequency_dictionaries(Some(progress_callback(
                progress_for_blocking.clone(),
            )))
            .map_err(|e| e.to_string())?,
        );

        let _ = progress_for_blocking.send(LoadingMessage::new("Loading ignore list..."));
        let ignore_list = Arc::new(Mutex::new(IgnoreList::load().map_err(|e| e.to_string())?));

        let jlpt = Arc::new(JlptDatabase::load());

        Ok(LanguageTools { tokenizer, frequency_manager, ignore_list, jlpt, known_interval })
    })
    .await
    .map_err(|e| e.to_string())?;

    match loaded {
        Ok(tools) => {
            let mut guard = state.lock().unwrap();
            apply_frequency_weights(&tools.frequency_manager, &guard.settings.frequency_weights);
            guard.language_tools = Some(tools);
            drop(guard);
            let _ = progress.send(LoadingMessage::clear());
            let _ = app.emit(names::LANGUAGE_TOOLS_STATUS, LanguageToolsStatus::Ready);
            Ok(())
        }
        Err(e) => {
            let _ = progress.send(LoadingMessage::clear());
            let _ = app.emit(names::LANGUAGE_TOOLS_STATUS, LanguageToolsStatus::Error(e.clone()));
            Err(e)
        }
    }
}

/// The engine's `get_app_data_dir()` creates the dir if missing. Opening from
/// Rust via the opener plugin bypasses JS scope — no extra capability needed.
#[tauri::command]
pub fn open_data_folder(app: AppHandle) -> Result<(), String> {
    let dir = persistence::get_app_data_dir();
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Static POS key/label list for filter UIs (keys match `settings.pos_filters`).
#[tauri::command]
pub fn get_pos_catalog() -> Vec<PosInfo> {
    POS::all().iter().copied().map(PosInfo::from_pos).collect()
}

/// The backend-owned settings (loaded at startup). One source of truth.
#[tauri::command]
pub fn get_settings(state: State<'_, Mutex<AppState>>) -> SettingsData {
    state.lock().unwrap().settings.clone()
}

/// Persist + replace the in-memory copy, propagating the bits that affect the
/// live tools (known-interval, frequency weights).
#[tauri::command]
pub fn save_settings(
    state: State<'_, Mutex<AppState>>,
    settings: SettingsData,
) -> Result<(), String> {
    persistence::save_json(&settings, "settings.json").map_err(|e| e.to_string())?;

    let mut guard = state.lock().unwrap();
    guard.settings = settings;
    let anki_interval = guard.settings.anki_interval;
    if let Some(tools) = guard.language_tools.as_mut() {
        tools.known_interval = anki_interval;
    }
    // Known-interval / frequency weights feed the knowledge summary; recompute it.
    guard.knowledge_dirty.store(true, Ordering::Relaxed);
    // `frequency_manager` is behind an `Arc` with interior mutability, so clone the
    // handle to drop the borrow on `guard` before reapplying weights.
    let manager = guard.language_tools.as_ref().map(|t| Arc::clone(&t.frequency_manager));
    let weights = guard.settings.frequency_weights.clone();
    drop(guard);
    if let Some(manager) = manager {
        apply_frequency_weights(&manager, &weights);
    }
    Ok(())
}
