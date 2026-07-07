//! Dictionary-manager commands (T064, issue #100): the "Recommended" section of
//! the Frequency Dictionaries modal — catalog with install/update state, one-click
//! install/update, and removal of any installed dictionary. All three end in
//! `dictionary::reload_and_swap`, so the live manager, per-term frequencies, and
//! knowledge summary stay consistent and `dictionaries-changed` fires.

use std::{
    collections::HashMap,
    fs,
    sync::Mutex,
};

use tauri::{
    ipc::Channel,
    AppHandle,
    State,
};
use yomine::{
    core::http,
    dictionary::frequency_manager::get_frequency_dict_dir,
    persistence,
};

use super::dictionary::reload_and_swap;
use crate::{
    dto::RecommendedDictionaryDto,
    events::LoadingMessage,
    recommended::{
        parse_manifest,
        RecommendedEntry,
        BAKED_MANIFEST,
        MANIFEST_URL,
    },
    state::AppState,
};

/// Fetch the recommended catalog and resolve each entry's state against the
/// loaded dictionaries. Remote manifest first (repo-hosted, updateable without a
/// release), baked copy as the offline/pre-publish fallback; entries with an
/// `index_url` get a live latest-revision check (best-effort). Caches the
/// catalog in `AppState` for `install_recommended_dictionary`.
#[tauri::command]
pub async fn get_recommended_dictionaries(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<RecommendedDictionaryDto>, String> {
    let installed: HashMap<String, String> = {
        let guard = state.lock().unwrap();
        guard
            .language_tools
            .as_ref()
            .map(|t| t.frequency_manager.dictionary_revisions())
            .unwrap_or_default()
    };

    // Manifest + update-index fetches are blocking reqwest calls — off the runtime.
    let entries =
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<RecommendedEntry>, String> {
            let mut entries = http::fetch_text(MANIFEST_URL)
                .ok()
                .and_then(|text| parse_manifest(&text).ok())
                .map_or_else(|| parse_manifest(BAKED_MANIFEST), Ok)?;
            for entry in &mut entries {
                let Some(url) = &entry.index_url else { continue };
                // Unreachable index just leaves the manifest's static revision.
                if let Some(rev) = http::fetch_text(url)
                    .ok()
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                    .and_then(|v| v.get("revision").and_then(|r| r.as_str()).map(String::from))
                {
                    entry.latest_revision = Some(rev);
                }
            }
            Ok(entries)
        })
        .await
        .map_err(|e| e.to_string())??;

    state.lock().unwrap().recommended_catalog = entries.clone();

    Ok(entries
        .into_iter()
        .map(|e| {
            let installed_revision = installed.get(&e.title).cloned();
            let status = match (&installed_revision, &e.latest_revision) {
                (None, _) => "not-installed",
                (Some(_), None) => "installed",
                (Some(inst), Some(latest)) if inst == latest => "up-to-date",
                _ => "update-available",
            };
            RecommendedDictionaryDto {
                name: e.name,
                title: e.title,
                description: e.description,
                installed_revision,
                latest_revision: e.latest_revision,
                status: status.to_string(),
            }
        })
        .collect())
}

/// Install or update a recommended dictionary (resolved by title from the cached
/// catalog): download the zip to a temp file, replace any installed artifacts of
/// the same title (an update's new zip name would otherwise extract beside the
/// old folder and load a duplicate), then reload + swap. Streams progress.
#[tauri::command]
pub async fn install_recommended_dictionary(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    title: String,
    progress: Channel<LoadingMessage>,
) -> Result<(), String> {
    let entry = {
        let guard = state.lock().unwrap();
        if guard.language_tools.is_none() {
            return Err("Language tools are still loading".to_string());
        }
        guard
            .recommended_catalog
            .iter()
            .find(|e| e.title == title)
            .cloned()
            .ok_or_else(|| "Unknown recommended dictionary — reopen the manager".to_string())?
    };

    let progress_dl = progress.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let dir = get_frequency_dict_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stem = sanitize_stem(&entry.title);
        let dest = dir.join(format!("{stem}.zip"));
        // Download to `.part` first so a failed update never deletes the working
        // copy (`.part` is inert: the loader only extracts `*.zip`).
        let tmp = dir.join(format!("{stem}.zip.part"));

        let _ = progress_dl.send(LoadingMessage::new(format!("Downloading {}...", entry.name)));
        let client = http::http_client().map_err(|e| e.to_string())?;
        let callback: Box<dyn Fn(String) + Send> = Box::new(move |message: String| {
            let _ = progress_dl.send(LoadingMessage::new(message));
        });
        http::download_with_progress(&client, &entry.download_url, &tmp, Some(callback.as_ref()))
            .map_err(|e| e.to_string())?;

        remove_dictionary_files(&entry.title)?;
        fs::rename(&tmp, &dest).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    reload_and_swap(&app, &state, &progress).await
}

/// Remove an installed dictionary — extracted folder (incl. its cache) + the zip
/// it came from — drop its persisted weight, and reload. Works for any installed
/// dictionary, not just recommended ones. Note: removing the *last* dictionary
/// makes the reload re-download the engine's default (JPDB) — engine behavior.
#[tauri::command]
pub async fn remove_dictionary(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    title: String,
    progress: Channel<LoadingMessage>,
) -> Result<(), String> {
    if state.lock().unwrap().language_tools.is_none() {
        return Err("Language tools are still loading".to_string());
    }

    let for_removal = title.clone();
    let removed = tauri::async_runtime::spawn_blocking(move || remove_dictionary_files(&for_removal))
        .await
        .map_err(|e| e.to_string())??;
    if !removed {
        return Err(format!("No installed dictionary titled '{title}' was found on disk"));
    }

    // The dict is gone; its persisted weight/enabled entry goes with it.
    let settings_to_save = {
        let mut guard = state.lock().unwrap();
        guard.settings.frequency_weights.remove(&title);
        guard.settings.clone()
    };
    persistence::save_json(&settings_to_save, "settings.json").map_err(|e| e.to_string())?;

    reload_and_swap(&app, &state, &progress).await
}

/// Delete every extracted folder in the freq-dict dir whose `index.json` title
/// matches, plus the same-stem zip that produced it (the extractor names the
/// folder after the zip stem). Returns whether anything was removed.
fn remove_dictionary_files(title: &str) -> Result<bool, String> {
    let dir = get_frequency_dict_dir();
    let Ok(entries) = fs::read_dir(&dir) else { return Ok(false) };
    let mut removed = false;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let matches = fs::read_to_string(path.join("index.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(|t| t == title))
            .unwrap_or(false);
        if !matches {
            continue;
        }
        fs::remove_dir_all(&path)
            .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
        let zip = path.with_extension("zip");
        if zip.exists() {
            let _ = fs::remove_file(&zip);
        }
        removed = true;
    }
    Ok(removed)
}

/// Strip filesystem-hostile characters from a zip stem (unicode like ㋕ is fine).
fn sanitize_stem(title: &str) -> String {
    title.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect()
}
