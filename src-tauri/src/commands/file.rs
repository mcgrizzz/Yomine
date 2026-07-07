//! File / mining commands (contracts/commands.md "File / mining").

use std::sync::{
    atomic::Ordering,
    Mutex,
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
    anki::comprehensibility::calculate_sentence_comprehension,
    core::{
        filename_parser,
        models::{
            SourceFile,
            SourceFileType,
        },
        pipeline::{
            apply_filters,
            process_sentences,
            process_source_file,
            AnkiFilter,
        },
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
    events::{
        names,
        AnkiStatus,
        ErrorPayload,
        LoadingMessage,
    },
    player_task::PlayerHandle,
    state::{
        AppState,
        FileData,
    },
};

/// egui uses id 3 for ad-hoc opened files; match it so downstream ids align.
const DEFAULT_SOURCE_FILE_ID: u32 = 3;

/// Build a `FileLoadResult` from the stored file state (sentences → DTOs).
pub(crate) fn load_result(file: &FileData) -> Option<FileLoadResult> {
    let source_file = file.source_file.clone()?;
    Some(FileLoadResult {
        source_file,
        terms: file.terms.clone(),
        sentences: file.sentences.iter().map(SentenceDto::from_sentence).collect(),
        file_comprehension: file.file_comprehension,
        anki_filter_active: !file.anki_known_lemmas.is_empty(),
        total_terms: file.base_terms.len(),
        ignored_terms: file.ignored_count,
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
    app: AppHandle,
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

    // Segmentation blocks the async runtime briefly, but the UI is a separate
    // webview process — nothing user-visible freezes.
    let (base_terms, filter_result, sentences, file_comprehension) =
        process_source_file(&source_file, &tools).await.map_err(|e| e.to_string())?;

    // Record the file in the shared `recent_files.json` (same store as egui) so it
    // appears on the landing state, mirroring egui's `add_recent_file`.
    record_recent_file(&source_file, filter_result.terms.len());

    // Lemmas Anki already knew — kept so an ignore-list change can re-filter
    // without re-querying Anki.
    let anki_known_lemmas =
        filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();

    let mut guard = state.lock().unwrap();
    guard.file = FileData {
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        anki_known_lemmas,
        ignored_count: filter_result.ignore_filtered.len(),
        sentences,
        file_comprehension,
        asbplayer_media_id: None,
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    // The table shows the cached Anki snapshot immediately; if Anki is live,
    // refresh against it in the background via `terms-refreshed`.
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if yomine::anki::api::get_version().await.is_ok() {
            if let Err(e) = live_refresh(&app_handle).await {
                let _ = app_handle.emit(
                    names::ERROR,
                    ErrorPayload {
                        title: "Refresh Error".into(),
                        message: "Unable to refresh terms".into(),
                        detail: Some(e),
                    },
                );
            }
        }
    });

    let _ = progress.send(LoadingMessage::clear());
    Ok(payload)
}

/// Fetch a media's subtitles from asbplayer and run them through the same
/// pipeline as a file (cue timings preserved, so seek/👁 work). Cues are also
/// saved as an `.srt` so the session lands in recents and reopens without
/// asbplayer. `track_numbers = None` loads all tracks.
#[tauri::command]
pub async fn load_asbplayer_media(
    app: AppHandle,
    player: State<'_, PlayerHandle>,
    media_id: String,
    track_numbers: Option<Vec<u32>>,
    title: String,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    load_asbplayer_into_state(&app, &player, media_id, track_numbers, title, Some(&progress)).await
}

/// The shared asbplayer-load path — the command above (picker, with progress)
/// and the follow-mode background loop (no progress channel) both use it.
pub(crate) async fn load_asbplayer_into_state(
    app: &AppHandle,
    player: &PlayerHandle,
    media_id: String,
    track_numbers: Option<Vec<u32>>,
    title: String,
    progress: Option<&Channel<LoadingMessage>>,
) -> Result<FileLoadResult, String> {
    let state = app.state::<Mutex<AppState>>();
    let tools = state
        .lock()
        .unwrap()
        .language_tools
        .clone()
        .ok_or_else(|| "Language tools are still loading".to_string())?;

    let send = |msg: &str| {
        if let Some(p) = progress {
            let _ = p.send(LoadingMessage::new(msg));
        }
    };
    send("Fetching subtitles from asbplayer...");
    let subtitles = player.get_subtitles(Some(media_id.clone()), track_numbers).await?;
    if subtitles.is_empty() {
        return Err("asbplayer returned no subtitles for this media — load a subtitle file in \
                    asbplayer first"
            .to_string());
    }

    // Save the cues as a real .srt in the app data dir (best-effort): the session
    // then lands in recent files and can be reopened later without asbplayer.
    let title =
        if title.trim().is_empty() { "asbplayer video".to_string() } else { title.clone() };
    let saved_path = save_subtitles_srt(&subtitles, &title);
    let source_file = SourceFile {
        id: DEFAULT_SOURCE_FILE_ID,
        source: Some("asbplayer".to_string()),
        file_type: if saved_path.is_some() {
            SourceFileType::SRT
        } else {
            SourceFileType::Other("asbplayer".to_string())
        },
        title,
        creator: None,
        original_file: saved_path.unwrap_or_else(|| format!("asbplayer://{media_id}")),
    };

    let sentences: Vec<_> = subtitles
        .iter()
        .enumerate()
        .filter_map(|(id, cue)| cue.to_sentence(id, source_file.id))
        .collect();
    if sentences.is_empty() {
        return Err("The subtitles were empty after cleanup".to_string());
    }

    send("Processing subtitles...");
    let (base_terms, filter_result, sentences, file_comprehension) =
        process_sentences(sentences, &tools).await.map_err(|e| e.to_string())?;

    // Only a real on-disk file belongs in recent files (reopening goes through
    // the normal parser; text cleaning matches what we just processed).
    if std::path::Path::new(&source_file.original_file).exists() {
        record_recent_file(&source_file, filter_result.terms.len());
    }

    let anki_known_lemmas =
        filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
    let mut guard = state.lock().unwrap();
    guard.file = FileData {
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        anki_known_lemmas,
        ignored_count: filter_result.ignore_filtered.len(),
        sentences,
        file_comprehension,
        asbplayer_media_id: Some(media_id),
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    // Same background live-Anki refresh as `process_file`.
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if yomine::anki::api::get_version().await.is_ok() {
            if let Err(e) = live_refresh(&app_handle).await {
                let _ = app_handle.emit(
                    names::ERROR,
                    ErrorPayload {
                        title: "Refresh Error".into(),
                        message: "Unable to refresh terms".into(),
                        detail: Some(e),
                    },
                );
            }
        }
    });

    if let Some(p) = progress {
        let _ = p.send(LoadingMessage::clear());
    }
    Ok(payload)
}

/// Re-partition the loaded terms against **live** Anki data and emit
/// `terms-refreshed`. Marks the knowledge summary dirty — the live fetch just
/// rewrote the vocab cache.
pub(crate) async fn live_refresh(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<AppState>>();
    let (tools, base_terms, mut sentences, mappings) = {
        let guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        // Nothing loaded → nothing to refresh (egui's RequestRefresh no-ops too).
        if guard.file.base_terms.is_empty() {
            return Ok(());
        }
        (
            tools,
            guard.file.base_terms.clone(),
            guard.file.sentences.clone(),
            guard.settings.anki_model_mappings.clone(),
        )
    };

    // Mirror egui's `anki_fetching = true` spinner while the live fetch runs.
    let _ = app.emit(names::ANKI_STATUS, AnkiStatus { connected: true, fetching: true });

    let outcome: Result<FileLoadResult, String> = async {
        let filter_result = apply_filters(base_terms, &tools, AnkiFilter::Live(mappings))
            .await
            .map_err(|e| e.to_string())?;

        // Reconstruct the full term set and recompute comprehension from it.
        let mut all_terms = Vec::new();
        all_terms.extend(filter_result.terms.iter().cloned());
        all_terms.extend(filter_result.anki_filtered.iter().cloned());
        all_terms.extend(filter_result.ignore_filtered.iter().cloned());
        for sentence in &mut sentences {
            calculate_sentence_comprehension(sentence, &all_terms);
        }
        let file_comprehension = if sentences.is_empty() {
            0.0
        } else {
            sentences.iter().map(|s| s.comprehension).sum::<f32>() / sentences.len() as f32
        };

        let mut guard = state.lock().unwrap();
        guard.file.anki_known_lemmas =
            filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
        guard.file.ignored_count = filter_result.ignore_filtered.len();
        guard.file.terms = filter_result.terms;
        guard.file.base_terms = all_terms;
        guard.file.sentences = sentences;
        guard.file.file_comprehension = file_comprehension;
        // Recompute coverage from the fresh vocab cache (egui resets
        // `knowledge_summary_attempted`).
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
        Ok(load_result(&guard.file).expect("refreshed file has a source_file"))
    }
    .await;

    let _ = app.emit(
        names::ANKI_STATUS,
        AnkiStatus { connected: outcome.is_ok(), fetching: false },
    );
    let payload = outcome?;
    let _ = app.emit(names::TERMS_REFRESHED, &payload);
    Ok(())
}

/// Manual "reapply ignorelist and Anki filters" (egui's top-bar 🔄 / F5 / Cmd+R
/// → `RequestRefresh`). The updated file arrives via the `terms-refreshed` event.
#[tauri::command]
pub async fn refresh_terms(app: AppHandle) -> Result<(), String> {
    live_refresh(&app).await
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

/// Write cues to `<app data>/asbplayer_subtitles/<sanitized title>.srt`
/// (overwriting — reloading the same video updates its recents entry in place).
/// Best-effort: a write failure just skips the recents integration.
fn save_subtitles_srt(
    subtitles: &[yomine::websocket::RemoteSubtitle],
    title: &str,
) -> Option<String> {
    let dir = yomine::persistence::get_app_data_dir().join("asbplayer_subtitles");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[asbplayer] Failed to create subtitle dir: {e}");
        return None;
    }
    let stem: String = title
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .take(80)
        .collect();
    let stem = stem.trim();
    let path = dir.join(format!("{}.srt", if stem.is_empty() { "asbplayer video" } else { stem }));
    match std::fs::write(&path, yomine::websocket::subtitles_to_srt(subtitles)) {
        Ok(()) => Some(path.display().to_string()),
        Err(e) => {
            eprintln!("[asbplayer] Failed to save subtitles to {}: {e}", path.display());
            None
        }
    }
}
