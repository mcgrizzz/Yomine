//! File / mining commands (contracts/commands.md "File / mining").

use std::{
    path::PathBuf,
    sync::{
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
    core::{
        filename_parser::{
            self,
            MediaType,
        },
        models::{
            Sentence,
            SourceFile,
            SourceFileType,
        },
        pipeline::{
            process_sentences,
            process_source_file,
        },
        recent_files::RecentFileEntry,
        text_filter,
    },
    media::{
        ffmpeg,
        subtitles::{
            self,
            SubtitleChoice,
        },
    },
    persistence::db,
};

use crate::{
    anki_sync,
    dto::{
        term_spans_by_sentence,
        EpubBookDto,
        EpubChapterDto,
        EpubPartDto,
        FileLoadResult,
        QueuedVideoDto,
        SentenceDto,
    },
    events::{
        names,
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
    // base_terms (not the filtered `terms`) so known/ignored words color too.
    let spans = term_spans_by_sentence(&file.base_terms, &file.anki_known_lemmas);
    Some(FileLoadResult {
        source_file,
        terms: file.terms.clone(),
        sentences: file
            .sentences
            .iter()
            .map(|s| SentenceDto::from_sentence(s, spans.get(&s.id).map_or(&[], Vec::as_slice)))
            .collect(),
        file_comprehension: file.file_comprehension,
        anki_filter_active: !file.anki_known_lemmas.is_empty(),
        total_terms: file.base_terms.len(),
        ignored_terms: file.ignored_count,
        batch_source: crate::batches::BatchSource::from_file(file).ok()?,
        local_video: file.local_video.as_ref().map(|p| p.display().to_string()),
        subtitle_tracks: file.subtitle_tracks.clone(),
        subtitle_track: file.subtitle_track.clone(),
    })
}

/// Construct a `SourceFile` from a filesystem path, parsing title/creator from the
/// filename the same way the egui file modal does. EPUBs prefer the metadata title
/// and carry the picker's selection label for the top bar and recents to render.
fn source_file_from_path(
    path: &str,
    epub_chapters: Option<Vec<usize>>,
    epub_label: Option<String>,
) -> SourceFile {
    let filename =
        std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
    let media_info = filename_parser::parse_filename(filename);
    let metadata = media_info.get_metadata_string();
    let file_type = SourceFileType::from_extension(path);
    let is_epub = matches!(file_type, SourceFileType::EPUB);
    let title = if is_epub {
        yomine::epub::book_title(path)
            .ok()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| media_info.display_title())
    } else {
        media_info.display_title()
    };
    SourceFile {
        id: DEFAULT_SOURCE_FILE_ID,
        source: None,
        file_type,
        title,
        creator: if metadata.is_empty() { None } else { Some(metadata) },
        original_file: path.to_string(),
        epub_chapters,
        epub_label: epub_label
            .filter(|_| is_epub)
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()),
    }
}

async fn pick_path(
    dialog: tauri_plugin_dialog::FileDialogBuilder<tauri::Wry>,
) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    dialog.pick_file(move |path| {
        let _ = tx.send(path);
    });
    let chosen = rx.await.map_err(|_| "file dialog closed unexpectedly".to_string())?;
    Ok(chosen.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

/// Native open dialog (FR: file selection). Returns the chosen paths, empty if cancelled.
#[tauri::command]
pub async fn open_file_dialog(app: AppHandle) -> Result<Vec<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(
            "Videos, subtitles & text",
            &[SourceFileType::supported_extensions(), subtitles::VIDEO_EXTENSIONS].concat(),
        )
        .add_filter("Videos", subtitles::VIDEO_EXTENSIONS)
        .add_filter("Subtitles & text", SourceFileType::supported_extensions())
        .pick_files(move |paths| {
            let _ = tx.send(paths);
        });
    let chosen = rx.await.map_err(|_| "file dialog closed unexpectedly".to_string())?;
    Ok(chosen
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
        .collect())
}

#[tauri::command]
pub async fn open_folder_dialog(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path);
    });
    let chosen = rx.await.map_err(|_| "folder dialog closed unexpectedly".to_string())?;
    Ok(chosen.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

/// The videos among opened files and folders, in the order the queue plays them.
#[tauri::command]
pub async fn list_videos(paths: Vec<PathBuf>) -> Vec<QueuedVideoDto> {
    subtitles::videos_in(&paths)
        .iter()
        .map(|path| {
            let path = path.display().to_string();
            match filename_parser::parse_filename(&path) {
                MediaType::TvShow { title, season, episode: Some(e), .. } => QueuedVideoDto {
                    path,
                    show: Some(title),
                    episode: Some(match season {
                        Some(s) => format!("S{s:02}E{e:02}"),
                        None => format!("Episode {e}"),
                    }),
                },
                _ => QueuedVideoDto { path, show: None, episode: None },
            }
        })
        .collect()
}

/// Book title + pickable chapters for the EPUB chapter picker.
#[tauri::command]
pub async fn get_epub_chapters(path: String) -> Result<EpubBookDto, String> {
    let (title, chapters) = yomine::epub::list_chapters(&path).map_err(|e| e.to_string())?;
    let seen = epub_history(&path);
    Ok(EpubBookDto {
        title,
        chapters: chapters
            .into_iter()
            .map(|c| EpubChapterDto {
                title: c.title,
                char_count: c.char_count,
                parts: c
                    .parts
                    .into_iter()
                    .map(|p| EpubPartDto {
                        id: p.id,
                        char_count: p.char_count,
                        seen: seen.contains(&p.id),
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// Part ids already mined for this book.
fn epub_history(path: &str) -> Vec<usize> {
    db::with(|conn| db::sources::epub_parts_seen(conn, path)).unwrap_or_else(|e| {
        eprintln!("{e}");
        Vec::new()
    })
}

/// Video picker for the MPV launcher (issue #89).
#[tauri::command]
pub async fn open_video_dialog(app: AppHandle) -> Result<Option<String>, String> {
    pick_path(
        app.dialog()
            .file()
            .add_filter("Video", &["mkv", "mp4", "avi", "webm", "mov", "m4v", "ts"])
            .add_filter("All files", &["*"]),
    )
    .await
}

/// Executable picker for the MPV launcher's "Locate mpv…" flow (issue #89).
#[tauri::command]
pub async fn open_executable_dialog(app: AppHandle) -> Result<Option<String>, String> {
    let dialog = app.dialog().file();
    #[cfg(windows)]
    let dialog = dialog.add_filter("Executable", &["exe"]);
    pick_path(dialog).await
}

/// Parse + segment + filter a source file (cached Anki snapshot, offline-safe) and
/// return the minable terms + sentence DTOs. Stores the result in `AppState`.
#[tauri::command]
pub async fn process_file(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    path: String,
    epub_chapters: Option<Vec<usize>>,
    epub_label: Option<String>,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    let source_file = source_file_from_path(&path, epub_chapters, epub_label);
    load_file(app, state, source_file, None, progress).await
}

/// A file opened as a video: its subtitles, and the id of the one to load.
struct OpenedVideo {
    path: PathBuf,
    tracks: Vec<SubtitleChoice>,
    track: String,
}

/// Opens a video by loading its subtitles (a file beside it, or an embedded track) with the
/// video paired. `track` picks a subtitle by id; `None` takes the Japanese one with the most
/// lines.
#[tauri::command]
pub async fn open_video(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    path: String,
    track: Option<String>,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    let _ = progress.send(LoadingMessage::new("Reading the video's subtitles..."));
    let ffmpeg_path = { state.lock().unwrap().settings.ffmpeg_path.clone() };
    let video = PathBuf::from(&path);
    let found = video.clone();
    let wanted = track.clone();
    // Extracting an embedded track reads the whole video, so files beside it come first,
    // then the probe's language and cue counts decide, and only the chosen track is
    // extracted.
    let (all, tracks, chosen) = tauri::async_runtime::spawn_blocking(move || {
        let beside = subtitles::sidecars(&found);
        let (all, ffmpeg) = if beside.iter().any(subtitles::is_candidate) {
            (beside, None)
        } else {
            let configured = Some(ffmpeg_path.trim()).filter(|p| !p.is_empty());
            let ffmpeg = ffmpeg::find(configured.map(std::path::Path::new)).ok_or(
                "Opening a video needs ffmpeg. Download it in Settings → Local Media.".to_string(),
            )?;
            let info = ffmpeg.probe(&found).map_err(|e| e.to_string())?;
            let mut all = beside;
            all.extend(subtitles::embedded(&found, &info).map_err(|e| e.to_string())?);
            (all, Some(ffmpeg))
        };
        let mut tracks: Vec<SubtitleChoice> =
            all.iter().filter(|t| subtitles::is_candidate(t)).cloned().collect();
        let loadable = || tracks.iter().filter(|t| t.path.is_some());
        let uncounted = loadable().count() > 1 && loadable().any(|t| t.lines == 0);
        if let (Some(ffmpeg), true, None) = (&ffmpeg, uncounted, &wanted) {
            subtitles::extract(ffmpeg, &found, &mut tracks).map_err(|e| e.to_string())?;
        }
        let chosen = match &wanted {
            Some(id) => tracks.iter().position(|t| &t.id == id && t.path.is_some()),
            None => subtitles::default_choice(&tracks)
                .and_then(|c| tracks.iter().position(|t| t.id == c.id)),
        };
        if let (Some(ffmpeg), Some(i)) = (&ffmpeg, chosen) {
            subtitles::extract(ffmpeg, &found, &mut tracks[i..=i]).map_err(|e| e.to_string())?;
        }
        Ok::<_, String>((all, tracks, chosen))
    })
    .await
    .map_err(|e| e.to_string())??;

    let chosen = chosen.map(|i| &tracks[i]);
    let Some(chosen) = chosen else {
        let name = video.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path);
        let others = subtitles::other_languages(&all);
        let has = if others.is_empty() {
            "no .srt or .ass file beside it with the same name, and no embedded text track".into()
        } else {
            format!("only {} subtitles", others.join(", "))
        };
        return Err(format!(
            "{name} has no Japanese subtitles to mine ({has}). Put a Japanese .srt or .ass \
             beside it with the same name."
        ));
    };
    let subtitle_path =
        chosen.path.clone().expect("chosen tracks have a file").display().to_string();
    let mut source_file = source_file_from_path(&subtitle_path, None, None);
    let named = source_file_from_path(&path, None, None);
    source_file.title = named.title;
    source_file.creator = named.creator;
    let track = chosen.id.clone();
    load_file(app, state, source_file, Some(OpenedVideo { path: video, tracks, track }), progress)
        .await
}

async fn load_file(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    source_file: SourceFile,
    video: Option<OpenedVideo>,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    let (tools, filters, anki_state, input_revision) = {
        let mut guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        let anki_state = guard.anki_state();
        (
            tools,
            text_filter::compile_filters(&guard.settings),
            anki_state,
            guard.input_revision.clone(),
        )
    };

    let _ = progress.send(LoadingMessage::new("Processing file..."));

    // Segmentation blocks the async runtime briefly, but the UI is a separate
    // webview process — nothing user-visible freezes.
    let (base_terms, filter_result, sentences, file_comprehension) =
        process_source_file(&source_file, &tools, &filters, anki_state)
            .await
            .map_err(|e| e.to_string())?;

    let opened =
        video.as_ref().map_or(source_file.original_file.clone(), |v| v.path.display().to_string());
    let paired = record_open(&source_file, &sentences, filter_result.terms.len(), &opened);
    let local_video = match &video {
        Some(video) => Some(video.path.clone()),
        None => paired
            .clone()
            .or_else(|| subtitles::sibling_video(std::path::Path::new(&source_file.original_file))),
    };
    if local_video.is_some() && local_video != paired {
        remember_video(&source_file, &sentences, local_video.as_deref());
    }
    let (subtitle_tracks, subtitle_track) =
        video.map_or((Vec::new(), None), |v| (v.tracks, Some(v.track)));

    // Lemmas Anki already knew — kept so an ignore-list change can re-filter
    // without re-querying Anki.
    let anki_known_lemmas =
        filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();

    let mut guard = state.lock().unwrap();
    if !Arc::ptr_eq(&guard.input_revision, &input_revision) {
        return Err("Dictionary or settings changed while loading; reopen the file".to_string());
    }
    guard.file = FileData {
        revision: Arc::new(()),
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        anki_known_lemmas,
        ignored_count: filter_result.ignore_filtered.len(),
        sentences,
        file_comprehension,
        asbplayer_media_id: None,
        asbplayer_subtitle_file: None,
        local_video,
        subtitle_tracks,
        subtitle_track,
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    // The table shows the cached Anki snapshot immediately; if Anki is live,
    // refresh against it in the background via `terms-refreshed`.
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if yomine::anki::reachable().await {
            if let Err(e) = anki_sync::sync(&app_handle, true).await {
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
    subtitle_file_name: Option<String>,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    crate::background::MANUAL_PICK.store(true, std::sync::atomic::Ordering::Relaxed);
    load_asbplayer_into_state(
        &app,
        &player,
        media_id,
        track_numbers,
        title,
        subtitle_file_name,
        Some(&progress),
    )
    .await
}

/// The current subtitle file's name, without extensions. asbplayer can name a file after the
/// tab's page title plus earlier file names (`Page - 04.srt Show - 08.srt`), and Crunchyroll
/// keeps an earlier episode's page title, so only the last name is this file. Yomine's own
/// saved copy loaded back (`ep.srt.srt`) keeps the same name, so its batch still matches.
pub(crate) fn subtitle_stem(name: &str) -> &str {
    const EXTENSIONS: [&str; 4] = [".srt", ".ass", ".ssa", ".vtt"];
    let lower = name.to_ascii_lowercase();
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < lower.len() {
        match EXTENSIONS.iter().find(|ext| lower[at..].starts_with(*ext)) {
            Some(ext) => {
                pieces.push(&name[start..at]);
                at += ext.len();
                start = at;
            }
            None => at += lower[at..].chars().next().map_or(1, char::len_utf8),
        }
    }
    pieces.push(&name[start..]);
    pieces
        .into_iter()
        .map(|piece| piece.trim_matches(|c: char| c.is_whitespace() || c == '-'))
        .rfind(|piece| !piece.is_empty())
        .unwrap_or(name.trim())
}

/// The shared asbplayer-load path — the command above (picker, with progress)
/// and the follow-mode background loop (no progress channel) both use it.
pub(crate) async fn load_asbplayer_into_state(
    app: &AppHandle,
    player: &PlayerHandle,
    media_id: String,
    track_numbers: Option<Vec<u32>>,
    title: String,
    subtitle_file_name: Option<String>,
    progress: Option<&Channel<LoadingMessage>>,
) -> Result<FileLoadResult, String> {
    let state = app.state::<Mutex<AppState>>();
    let (tools, filters, anki_state, input_revision) = {
        let mut guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        let anki_state = guard.anki_state();
        (
            tools,
            text_filter::compile_filters(&guard.settings),
            anki_state,
            guard.input_revision.clone(),
        )
    };

    let file_name = subtitle_file_name.filter(|n| !n.trim().is_empty());
    {
        let guard = state.lock().unwrap();
        if guard.file.asbplayer_media_id.as_deref() == Some(media_id.as_str())
            && file_name.is_some()
            && guard.file.asbplayer_subtitle_file == file_name
        {
            if let Some(payload) = load_result(&guard.file) {
                return Ok(payload);
            }
        }
    }

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

    // Save the cues as a real .srt (best-effort): the session then lands in
    // recents and reopens without asbplayer.
    let title = if title.trim().is_empty() { "asbplayer video".to_string() } else { title };
    let (stem, display_title, creator) = match &file_name {
        Some(name) => {
            let stem = subtitle_stem(name).to_string();
            let media_info = filename_parser::parse_filename(&format!("{stem}.srt"));
            let metadata = media_info.get_metadata_string();
            (
                stem,
                media_info.display_title(),
                if metadata.is_empty() { None } else { Some(metadata) },
            )
        }
        None => (title.clone(), title, None),
    };
    let saved_path = save_subtitles_srt(&subtitles, &stem);
    let source_file = SourceFile {
        id: DEFAULT_SOURCE_FILE_ID,
        source: Some("asbplayer".to_string()),
        file_type: if saved_path.is_some() {
            SourceFileType::SRT
        } else {
            SourceFileType::Other("asbplayer".to_string())
        },
        title: display_title,
        creator,
        original_file: saved_path.unwrap_or_else(|| format!("asbplayer://{media_id}")),
        epub_chapters: None,
        epub_label: None,
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
        process_sentences(sentences, &tools, &filters, anki_state)
            .await
            .map_err(|e| e.to_string())?;

    let local_video = record_open(
        &source_file,
        &sentences,
        filter_result.terms.len(),
        &source_file.original_file,
    );

    let anki_known_lemmas =
        filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
    let mut guard = state.lock().unwrap();
    if !Arc::ptr_eq(&guard.input_revision, &input_revision) {
        return Err(
            "Dictionary or settings changed while loading; reopen the subtitles".to_string()
        );
    }
    guard.file = FileData {
        revision: Arc::new(()),
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        anki_known_lemmas,
        ignored_count: filter_result.ignore_filtered.len(),
        sentences,
        file_comprehension,
        asbplayer_media_id: Some(media_id),
        asbplayer_subtitle_file: file_name,
        local_video,
        subtitle_tracks: Vec::new(),
        subtitle_track: None,
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    // Same background live-Anki refresh as `process_file`.
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if yomine::anki::reachable().await {
            if let Err(e) = anki_sync::sync(&app_handle, true).await {
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

/// Manual "reapply ignorelist and Anki filters" (egui's top-bar 🔄 / F5 / Cmd+R
/// → `RequestRefresh`). The updated file arrives via the `terms-refreshed` event.
#[tauri::command]
pub async fn refresh_terms(app: AppHandle) -> Result<(), String> {
    anki_sync::sync(&app, true).await?;
    match anki_sync::refresh(&app, anki_sync::Refresh::Full).await? {
        anki_sync::RefreshOutcome::Done => Ok(()),
        anki_sync::RefreshOutcome::NoVocab => {
            Err("Anki returned no cards — check the note types mapped in Anki settings".to_string())
        }
    }
}

/// Re-fetch the currently loaded file (e.g. after a UI reload). `null` if none.
#[tauri::command]
pub fn get_terms(state: State<'_, Mutex<AppState>>) -> Option<FileLoadResult> {
    load_result(&state.lock().unwrap().file)
}

/// Re-run the full pipeline on the loaded file from its on-disk source
#[tauri::command]
pub async fn reload_current_file(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    progress: Channel<LoadingMessage>,
) -> Result<FileLoadResult, String> {
    let (tools, filters, source_file, media_id, subtitle_file, anki_state, input_revision) = {
        let mut guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        let source_file =
            guard.file.source_file.clone().ok_or_else(|| "No file is loaded".to_string())?;
        let anki_state = guard.anki_state();
        (
            tools,
            text_filter::compile_filters(&guard.settings),
            source_file,
            guard.file.asbplayer_media_id.clone(),
            guard.file.asbplayer_subtitle_file.clone(),
            anki_state,
            guard.input_revision.clone(),
        )
    };
    if !std::path::Path::new(&source_file.original_file).exists() {
        return Err(
            "The loaded file no longer exists on disk — reload it from its source".to_string()
        );
    }

    let _ = progress.send(LoadingMessage::new("Reprocessing file..."));
    let (base_terms, filter_result, sentences, file_comprehension) =
        process_source_file(&source_file, &tools, &filters, anki_state)
            .await
            .map_err(|e| e.to_string())?;

    let anki_known_lemmas =
        filter_result.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
    let mut guard = state.lock().unwrap();
    if !Arc::ptr_eq(&guard.input_revision, &input_revision) {
        return Err("Dictionary or settings changed while loading; reload the file".to_string());
    }
    guard.file = FileData {
        revision: Arc::new(()),
        source_file: Some(source_file),
        terms: filter_result.terms,
        base_terms,
        anki_known_lemmas,
        ignored_count: filter_result.ignore_filtered.len(),
        sentences,
        file_comprehension,
        asbplayer_media_id: media_id,
        asbplayer_subtitle_file: subtitle_file,
        local_video: guard.file.local_video.clone(),
        subtitle_tracks: guard.file.subtitle_tracks.clone(),
        subtitle_track: guard.file.subtitle_track.clone(),
    };
    let payload = load_result(&guard.file).expect("file just stored has a source_file");
    drop(guard);

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if yomine::anki::reachable().await {
            if let Err(e) = anki_sync::sync(&app_handle, true).await {
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

/// Records the load in history: the source, the recent-files entry, and any EPUB
/// parts, and returns the video paired with the source. Failures are only logged, so
/// history never fails an otherwise-good load.
fn record_open(
    source_file: &SourceFile,
    sentences: &[Sentence],
    term_count: usize,
    // What the user opened: the video, for subtitles loaded from one.
    opened: &str,
) -> Option<PathBuf> {
    let fingerprint = crate::batches::BatchSource::new(source_file, sentences).fingerprint;
    let info = db::sources::SourceInfo {
        fingerprint: &fingerprint,
        kind: match source_file.file_type {
            SourceFileType::SRT | SourceFileType::SSA => "subtitles",
            SourceFileType::TXT => "text",
            SourceFileType::EPUB => "epub",
            SourceFileType::Other(_) => "other",
        },
        title: &source_file.title,
        creator: source_file.creator.as_deref(),
        char_count: sentences.iter().map(|s| s.text.chars().count()).sum(),
        runtime_ms: sentences
            .iter()
            .filter_map(|s| s.timestamp.as_ref())
            .map(|t| (t.to_secs().1 * 1000.0).round() as i64)
            .max(),
    };
    let path = opened;
    let open = db::sources::Open {
        path,
        title: &source_file.title,
        label: source_file.epub_label.as_deref(),
        creator: source_file.creator.as_deref(),
        term_count: Some(term_count as i64),
        file_size: std::fs::metadata(path).map(|m| m.len() as i64).ok(),
        opened_at: db::now_ms(),
    };
    let parts = source_file.epub_chapters.as_deref().unwrap_or_default();
    let recorded = db::with(|conn| {
        db::sources::record_open(conn, &info, &open, parts)?;
        db::sources::video(conn, &fingerprint)
    });
    recorded
        .unwrap_or_else(|e| {
            eprintln!("Failed to record the file in history: {e}");
            None
        })
        .map(PathBuf::from)
}

fn remember_video(
    source_file: &SourceFile,
    sentences: &[Sentence],
    video: Option<&std::path::Path>,
) {
    let fingerprint = crate::batches::BatchSource::new(source_file, sentences).fingerprint;
    let path = video.map(|v| v.display().to_string());
    if let Err(e) = db::with(|conn| db::sources::set_video(conn, &fingerprint, path.as_deref())) {
        eprintln!("Failed to remember the paired video: {e}");
    }
}

/// Pairs a video with the loaded file, or unpairs it with `None`. The pairing is kept
/// with the source, so reopening the same subtitles finds the video again.
#[tauri::command]
pub fn pair_video(
    state: State<'_, Mutex<AppState>>,
    path: Option<String>,
) -> Result<Option<FileLoadResult>, String> {
    let mut guard = state.lock().unwrap();
    let fingerprint = crate::batches::BatchSource::from_file(&guard.file)?.fingerprint;
    if path.as_ref().is_some_and(|p| !std::path::Path::new(p).is_file()) {
        return Err("That video no longer exists".into());
    }
    db::with(|conn| db::sources::set_video(conn, &fingerprint, path.as_deref()))?;
    guard.file.local_video = path.map(PathBuf::from);
    Ok(load_result(&guard.file))
}

/// Recent files for the landing state, most recent first: each path's latest load,
/// for paths that still exist.
#[tauri::command]
pub fn get_recent_files() -> Vec<RecentFileEntry> {
    db::with(|conn| db::sources::recent(conn, 50))
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            Vec::new()
        })
        .into_iter()
        .filter(RecentFileEntry::file_exists)
        .collect()
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
    let stem: String =
        title.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).take(80).collect();
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

#[cfg(test)]
mod tests {
    use super::subtitle_stem;

    #[test]
    fn a_resaved_copy_keeps_the_original_stem() {
        for name in ["Show - 04.srt", "Show - 04.srt.srt.srt", "Show - 04.SRT.ass"] {
            assert_eq!(subtitle_stem(name), "Show - 04");
        }
        assert_eq!(subtitle_stem("Show v1.2.srt"), "Show v1.2");
        assert_eq!(subtitle_stem("Page - Watch on - Show - 04.srt Show - 08.srt"), "Show - 08");
        assert_eq!(subtitle_stem("Page - Show - 04.srt Show - 08.srt.srt"), "Show - 08");
    }
}
