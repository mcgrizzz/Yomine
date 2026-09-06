//! Frequency-dictionary commands (contracts/commands.md "Frequency dictionaries").

use std::{
    collections::HashMap,
    sync::{
        atomic::Ordering,
        Arc,
        Mutex,
    },
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

/// Update one dictionary's weight/enabled/hidden and persist it. The manager gets
/// `weight.max(0.1)`; settings keep the raw value (egui parity).
#[tauri::command]
pub async fn set_dictionary_state(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    name: String,
    weight: f32,
    enabled: bool,
    hidden: bool,
) -> Result<(), String> {
    set_dictionary_states(
        app,
        state,
        HashMap::from([(name, FrequencyDictionarySetting { weight, enabled, hidden })]),
    )
    .await
}

/// Commit all modal edits together, then reprocess the loaded file once.
#[tauri::command]
pub async fn set_dictionary_states(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    mut updates: HashMap<String, FrequencyDictionarySetting>,
) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }
    for setting in updates.values() {
        if !setting.weight.is_finite() || setting.weight < 0.0 {
            return Err("Dictionary weights must be finite and nonnegative".into());
        }
    }
    for setting in updates.values_mut() {
        setting.enabled &= !setting.hidden;
    }
    {
        let mut guard = state.lock().unwrap();
        let manager = guard
            .language_tools
            .as_ref()
            .map(|tools| Arc::clone(&tools.frequency_manager))
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        let mut settings_to_save = guard.settings.clone();
        settings_to_save.frequency_weights.extend(updates.clone());
        let previous_states = manager.dictionary_states().unwrap_or_default();
        let states = updates
            .into_iter()
            .map(|(name, setting)| {
                (
                    name,
                    frequency_manager::DictionaryState {
                        weight: setting.weight.max(0.1),
                        enabled: setting.enabled,
                    },
                )
            })
            .collect();
        manager.set_dictionary_states(&states).map_err(|e| e.to_string())?;
        if let Err(error) = persistence::save_json(&settings_to_save, "settings.json") {
            // A failed write must not leave live settings different from disk.
            manager.set_dictionary_states(&previous_states).map_err(|e| e.to_string())?;
            guard.invalidate_anki_cache();
            return Err(error.to_string());
        }
        guard.settings = settings_to_save;
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
        guard.invalidate_dictionary_evidence();
    }

    let _ = app.emit(names::DICTIONARIES_CHANGED, ());
    refresh_loaded_file(&app).await?;
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
/// tools — the shared tail of every dictionary mutation. Reprocesses the loaded
/// sentences because dictionary evidence affects segmentation and promotion as
/// well as ranking and Anki matching.
pub(crate) async fn reload_and_swap(
    app: &AppHandle,
    state: &State<'_, Mutex<AppState>>,
    progress: &Channel<LoadingMessage>,
) -> Result<(), String> {
    let generation = {
        let mut guard = state.lock().unwrap();
        guard.invalidate_dictionary_evidence();
        guard.input_revision.clone()
    };
    let progress_for_blocking = progress.clone();
    let reloaded = tauri::async_runtime::spawn_blocking(move || {
        let _ =
            progress_for_blocking.send(LoadingMessage::new("Reloading frequency dictionaries..."));
        let callback = Box::new(move |message: String| {
            let _ = progress_for_blocking.send(LoadingMessage::new(message));
        });
        frequency_manager::process_frequency_dictionaries(Some(callback)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|result| result);
    let _ = progress.send(LoadingMessage::clear());
    let manager = match reloaded {
        Ok(manager) => Arc::new(manager),
        Err(error) => {
            let mut guard = state.lock().unwrap();
            if Arc::ptr_eq(&guard.input_revision, &generation) {
                guard.dictionary_refresh_pending = None;
            }
            return Err(error);
        }
    };

    {
        let mut guard = state.lock().unwrap();
        if !Arc::ptr_eq(&guard.input_revision, &generation) {
            return Ok(());
        }
        apply_frequency_weights(&manager, &guard.settings.frequency_weights);
        let Some(tools) = guard.language_tools.as_mut() else {
            return Err("Language tools are still loading".to_string());
        };
        tools.frequency_manager = Arc::clone(&manager);
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
    }

    let _ = app.emit(names::DICTIONARIES_CHANGED, ());
    refresh_loaded_file(app).await?;
    Ok(())
}

/// Recompute from the in-memory, already-cleaned sentence text. This also works
/// for unsaved asbplayer subtitles and does not reapply non-idempotent text
/// filters, query live Anki, or write the user's Anki snapshot.
async fn refresh_loaded_file(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let (tools, sentences, anki, file_revision, input_revision) = {
        let mut guard = state.lock().unwrap();
        if guard.file.source_file.is_none() {
            guard.dictionary_refresh_pending = None;
            return Ok(());
        }
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        let anki = guard.anki_state();
        (
            tools,
            sentences_for_refresh(&guard.file.sentences),
            anki,
            guard.file.revision.clone(),
            guard.input_revision.clone(),
        )
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(yomine::core::pipeline::process_sentences(
            sentences,
            &tools,
            &[],
            anki,
        ))
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|result| result);

    let mut guard = state.lock().unwrap();
    if Arc::ptr_eq(&guard.input_revision, &input_revision) {
        guard.dictionary_refresh_pending = None;
    }
    // A later dictionary/settings change or completed file load owns the UI now.
    if !guard.file_update_is_current(&file_revision, &input_revision) {
        return Ok(());
    }
    let (base_terms, filtered, sentences, comprehension) = result?;
    let file = &mut guard.file;
    file.revision = Arc::new(());
    file.anki_known_lemmas = filtered.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
    file.ignored_count = filtered.ignore_filtered.len();
    file.terms = filtered.terms;
    file.base_terms = base_terms;
    file.sentences = sentences;
    file.file_comprehension = comprehension;
    // Emit while holding the lock so a later file result cannot overtake this event.
    if let Some(payload) = super::file::load_result(file) {
        let _ = app.emit(names::TERMS_REFRESHED, payload);
    }
    Ok(())
}

fn sentences_for_refresh(sentences: &[yomine::core::Sentence]) -> Vec<yomine::core::Sentence> {
    sentences
        .iter()
        .cloned()
        .map(|mut sentence| {
            sentence.segments.clear();
            sentence.comprehension = 0.0;
            sentence
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use yomine::{
        core::Sentence,
        dictionary::{
            frequency_manager::FrequencyManager,
            token_dictionary::DictType,
        },
        segmentation::tokenizer::{
            extract_words,
            init_vibrato,
        },
    };

    use super::sentences_for_refresh;

    #[test]
    #[ignore = "requires installed UniDic; run cargo test -p yomine-tauri --lib -- --include-ignored"]
    fn refreshing_processed_sentences_matches_a_fresh_extraction() {
        let tokenizer = init_vibrato(&DictType::Unidic, None).expect("UniDic must be available");
        let manager = FrequencyManager::from_dictionaries(Vec::new());
        let source = vec![Sentence {
            id: 7,
            source_id: 3,
            text: "要らないです。形態化させて。".to_string(),
            segments: Vec::new(),
            timestamp: None,
            comprehension: 0.0,
        }];
        let mut fresh = source.clone();
        let expected = extract_words(tokenizer.new_worker(), &mut fresh, &manager);
        assert!(!fresh[0].segments.is_empty());

        let mut refreshed = fresh.clone();
        // Repeated dictionary edits must never accumulate old segmentation.
        for _ in 0..2 {
            refreshed[0].comprehension = 1.0;
            refreshed = sentences_for_refresh(&refreshed);
            assert_eq!(refreshed[0].comprehension, 0.0);
            let actual = extract_words(tokenizer.new_worker(), &mut refreshed, &manager);
            assert_eq!(refreshed[0].segments, fresh[0].segments);
            assert_eq!(refreshed[0].text, source[0].text);
            assert_eq!(refreshed[0].id, source[0].id);
            assert_eq!(refreshed[0].source_id, source[0].source_id);
            let citations = |terms: &[yomine::core::Term]| {
                terms
                    .iter()
                    .map(|term| (term.lemma_form.clone(), term.lemma_reading.clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(citations(&actual), citations(&expected));
        }
    }
}
