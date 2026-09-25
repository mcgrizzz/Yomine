//! One-click mining (issue #105) + mined-state tracking (issue #3). The note
//! is always created via AnkiConnect from Yomitan-rendered fields; the
//! asbplayer path then enriches it (audio/screenshot) via a note-targeted
//! `mine-subtitle` update.

use std::{
    sync::Mutex,
    time::Duration,
};

use tauri::{
    ipc::Channel,
    State,
};
use yomine::{
    anki::{
        api as anki_api,
        mined,
    },
    yomitan,
};

use crate::{
    batches::{
        self,
        BatchItem,
        BatchRecord,
        BatchSource,
        Failure,
        FailureKind,
        FailureScope,
        Fallback,
        MediaState,
        Outcome,
    },
    dto::{
        CardFormatDto,
        DefinitionEntryDto,
        MineResultDto,
        MinedStateDto,
        YomitanStatusDto,
    },
    events::LoadingMessage,
    player_task::PlayerHandle,
    state::AppState,
};

const SEEK_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);
const SEEK_CONFIRM_POLL: Duration = Duration::from_millis(250);
/// Extra wait past the cue's duration for asbplayer to finish recording.
const RECORD_BUFFER: Duration = Duration::from_millis(1500);
const MEDIA_VERIFY_TIMEOUT: Duration = Duration::from_secs(6);
const MEDIA_VERIFY_POLL: Duration = Duration::from_millis(500);
/// Cloze refinement is optional; this caps how long it can delay a mine.
const MATCH_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

#[tauri::command]
pub async fn mine_term(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    term: String,
    reading: Option<String>,
    surface: String,
    sentence: String,
    timestamp_secs: Option<f32>,
    timestamp_end_secs: Option<f32>,
    timestamp_label: Option<String>,
    via: String,
    entry_index: Option<usize>,
    format_name: Option<String>,
    progress: Channel<LoadingMessage>,
) -> Result<MineResultDto, String> {
    let _operation =
        batches::OPERATION.try_lock().map_err(|_| "Mining or undo is already running")?;
    let (yomitan_url, media_id) = {
        let guard = state.lock().unwrap();
        (guard.settings.yomitan_url.clone(), guard.file.asbplayer_media_id.clone())
    };
    let item = BatchItem {
        key: reading.map(|r| format!("{term} {r}")).unwrap_or_default(),
        lemma: term,
        surface,
        sentence,
        timestamp: timestamp_secs.map(|start_secs| crate::dto::TimeStampDto {
            start_secs,
            end_secs: timestamp_end_secs.unwrap_or(start_secs),
            start_label: timestamp_label.unwrap_or_default(),
            end_label: String::new(),
        }),
        entry_index,
        format_name,
        scan_text: None,
        adhoc: false,
        mine_media: via == "asbplayer",
        outcome: Outcome::Unattempted,
    };
    let options = MineOptions { record: true, require_dictionary_media: true };
    let mut result =
        mine(&yomitan_url, &item, options, &progress, None).await.map_err(|e| e.message)?;

    // Enrichment failures don't undo the mine (the note exists) — warn instead.
    if let Some(id) = result.note_id.filter(|_| item.mine_media) {
        let warning = if target_lacks_subtitles(&player, media_id.as_deref()).await {
            Some(
                "asbplayer has no subtitles loaded on the loaded video — card created without \
                 audio/screenshot"
                    .to_string(),
            )
        } else {
            record_item(&player, id, media_id, &item, &progress)
                .await
                .err()
                .map(|e| format!("Card created, but media wasn't added: {e}"))
        };
        result.media_missing = warning.is_some();
        result.warning = warning;
    }
    Ok(result)
}

#[derive(serde::Deserialize, Clone, Copy)]
pub struct MineOptions {
    record: bool,
    require_dictionary_media: bool,
}

#[derive(serde::Serialize)]
pub struct BatchStep {
    batch: BatchRecord,
    failure: Option<Failure>,
    preview_file: Option<String>,
}

#[tauri::command]
pub async fn mine_batch_item(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    batch_id: String,
    item_index: usize,
    media_target: Option<String>,
    options: MineOptions,
    progress: Channel<LoadingMessage>,
) -> Result<BatchStep, String> {
    let _operation =
        batches::OPERATION.try_lock().map_err(|_| "Mining or undo is already running")?;
    let mut batch = batches::load(&batch_id)?;
    let item = batch.items.get(item_index).ok_or("Batch item not found")?.clone();
    let (url, loaded_target) = {
        let state = state.lock().unwrap();
        if !batch.source.matches(&BatchSource::from_file(&state.file)?) {
            return Err("Load the original source before retrying this batch".into());
        }
        (state.settings.yomitan_url.clone(), state.file.asbplayer_media_id.clone())
    };
    let target = loaded_target.or(media_target);
    let mut preview_file = None;
    let result = async {
        match item.outcome {
            Outcome::Unattempted | Outcome::Failed { .. } => {
                mine(&url, &item, options, &progress, Some((&mut batch, item_index))).await?;
            }
            Outcome::Created {
                note_id,
                media: MediaState::Pending | MediaState::Failed | MediaState::Skipped,
                ..
            } => {
                validate_media_target(&player, target.as_deref()).await?;
                batches::checkpoint(
                    &mut batch,
                    item_index,
                    Outcome::Created { note_id, media: MediaState::Pending, error: None },
                )?;
                let result = record_item(&player, note_id, target, &item, &progress).await;
                let failure = match result {
                    Ok(image) => {
                        preview_file = image;
                        None
                    }
                    Err(e) => Some(e.failure()),
                };
                batches::checkpoint(
                    &mut batch,
                    item_index,
                    Outcome::Created {
                        note_id,
                        media: if failure.is_some() {
                            MediaState::Failed
                        } else {
                            MediaState::Complete
                        },
                        error: failure.clone(),
                    },
                )?;
                if let Some(failure) = failure {
                    return Err(failure);
                }
            }
            _ => {
                return Err(Failure::new(
                    "Batch recovery",
                    FailureScope::Stop,
                    "This item cannot be retried automatically",
                ))
            }
        }
        Ok(())
    }
    .await;
    let mut failure = result.err();
    if let Some(error) = &failure {
        if error.scope != FailureScope::Stop {
            let outcome = match &batch.items[item_index].outcome {
                Outcome::Unattempted | Outcome::Failed { .. } => {
                    Some(Outcome::Failed { error: error.clone() })
                }
                Outcome::Created { note_id, media, .. } => Some(Outcome::Created {
                    note_id: *note_id,
                    media: *media,
                    error: Some(error.clone()),
                }),
                _ => None,
            };
            if let Some(outcome) = outcome {
                if let Err(storage) = batches::checkpoint(&mut batch, item_index, outcome) {
                    failure = Some(storage);
                }
            }
        }
    }
    Ok(BatchStep { batch, failure, preview_file })
}

async fn validate_media_target(player: &PlayerHandle, target: Option<&str>) -> Result<(), Failure> {
    let status =
        player.status().await.map_err(|e| Failure::new("asbplayer", FailureScope::Shared, e))?;
    if status.ws_clients == 0 {
        return Err(Failure::new(
            "asbplayer",
            FailureScope::Shared,
            "asbplayer is disconnected. Open the video and reconnect the extension, then retry.",
        ));
    }
    let media = player
        .get_bound_media()
        .await
        .map_err(|e| Failure::new("asbplayer", FailureScope::Unknown, e))?;
    let target = media.iter().find(|m| Some(m.id.as_str()) == target).ok_or_else(|| {
        Failure::new(
            "asbplayer",
            FailureScope::Shared,
            "The original video is not available. Reopen it in asbplayer before retrying.",
        )
    })?;
    if !target.active || target.loaded_subtitles.is_empty() {
        return Err(Failure::new(
            "asbplayer",
            FailureScope::Shared,
            "Activate the video's tab and load its subtitles in asbplayer, then retry.",
        ));
    }
    Ok(())
}

/// Yomitan's first entry can be a different word with the same spelling (止める as やめる when
/// the sentence reads とめる), so a row picks the entry matching its own reading.
async fn default_entry(yomitan_url: &str, item: &BatchItem, term: &str) -> usize {
    // A row's key is `termKey`: "{lemma} {reading}".
    let reading = match item.key.split_once(' ') {
        Some((lemma, reading)) if !item.adhoc && lemma == item.lemma => reading,
        _ => return 0,
    };
    tokio::time::timeout(
        MATCH_LOOKUP_TIMEOUT,
        yomitan::entry_index_for(yomitan_url, term, &item.lemma, reading),
    )
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
}

async fn mine(
    yomitan_url: &str,
    item: &BatchItem,
    options: MineOptions,
    progress: &Channel<LoadingMessage>,
    mut batch: Option<(&mut BatchRecord, usize)>,
) -> Result<MineResultDto, Failure> {
    let term = item.scan_text.as_ref().unwrap_or(&item.lemma).clone();
    let surface = item.surface.clone();
    let sentence = item.sentence.clone();
    let entry_index = item.entry_index;
    let format_name = item.format_name.clone();
    let timestamp_secs = item.timestamp.as_ref().map(|t| t.start_secs);
    let via = if item.mine_media { "asbplayer" } else { "direct" }.to_string();
    let entry_index = match entry_index {
        Some(index) => index,
        None => default_entry(yomitan_url, item, &term).await,
    };

    let _ = progress.send(LoadingMessage::new(format!("Rendering 「{}」 with Yomitan…", term)));
    let formats = yomitan::get_term_card_formats(yomitan_url)
        .await
        .map_err(|e| Failure::new("Yomitan connection", FailureScope::Shared, e))?;
    let format = match &format_name {
        Some(name) => formats.iter().find(|f| &f.name == name).ok_or_else(|| {
            Failure::new(
                "Card format",
                FailureScope::Shared,
                format!("Yomitan card format \"{}\" no longer exists", name),
            )
        })?,
        None => formats.first().ok_or_else(|| {
            Failure::new(
                "Card format",
                FailureScope::Shared,
                "Configure a term card format in Yomitan",
            )
        })?,
    };
    let format_markers = yomitan::collect_markers(format);
    let mut markers = format_markers.clone();
    for extra in ["expression", "reading"] {
        if !markers.iter().any(|m| m == extra) {
            markers.push(extra.to_string());
        }
    }
    let rendered =
        yomitan::render_fields(yomitan_url, &term, &markers, entry_index as u32 + 1, true)
            .await
            .map_err(|e| Failure::new("Rendering card", FailureScope::Unknown, e))?;

    let empty = std::collections::HashMap::new();
    let marker_values = rendered.fields.get(entry_index).unwrap_or(&empty);
    // Only the format's own markers: expression renders for every entry that exists.
    if format_markers.iter().all(|m| marker_values.get(m).is_none_or(|v| v.trim().is_empty())) {
        return Err(Failure::new(
            "Dictionary entry",
            FailureScope::Item,
            format!("Yomitan has no dictionary entry for 「{}」", term),
        ));
    }
    let key = mined::entry_key(
        marker_values.get("expression").map(String::as_str).unwrap_or_default(),
        marker_values.get("reading").map(String::as_str).unwrap_or_default(),
    );

    // Cloze highlighting must match the text as it appears in the sentence: an
    // inflected occurrence (沈めて) never contains the lemma (沈める).
    let matched = tokio::time::timeout(
        MATCH_LOOKUP_TIMEOUT,
        yomitan::matched_source(yomitan_url, &term, entry_index),
    )
    .await
    .ok()
    .flatten();
    let cloze_term = matched
        .as_deref()
        .filter(|m| sentence.contains(m) && (m.starts_with(&surface) || surface.starts_with(*m)))
        .or(if !surface.is_empty() && sentence.contains(&surface) {
            Some(surface.as_str())
        } else {
            None
        })
        .unwrap_or(term.as_str());
    let ctx = yomitan::SentenceContext { sentence: &sentence, term: cloze_term };
    let fields = yomitan::assemble_fields(format, marker_values, Some(ctx));
    if fields.is_empty() {
        return Err(Failure::new(
            "Rendering card",
            FailureScope::Item,
            format!("Yomitan rendered no card content for 「{}」", term),
        ));
    }

    let _ = progress.send(LoadingMessage::new("Creating Anki note…"));

    for media in rendered.audio_media.iter().chain(rendered.dictionary_media.iter()) {
        let response = anki_api::store_media_file(&media.anki_filename, &media.content)
            .await
            .map_err(|e| Failure::new("Uploading media", FailureScope::Shared, e))?;
        match response.error {
            Some(error) if options.require_dictionary_media => {
                return Err(Failure::new(
                    "Uploading media",
                    FailureScope::Unknown,
                    format!("{}: {}", media.anki_filename, error),
                )
                .with_fallback(Fallback::WithoutDictionaryMedia))
            }
            Some(error) => eprintln!("storeMediaFile {}: {}", media.anki_filename, error),
            None => {}
        }
    }

    // No timestamp = no attachable media (EPUB/TXT); tags the note for a future re-mine flow.
    let mut tags = vec!["yomine".to_string()];
    if timestamp_secs.is_none() {
        tags.push("yomine::no-media".to_string());
    }
    if let Some((record, index)) = &mut batch {
        batches::checkpoint(record, *index, Outcome::Attempting)?;
    }
    let response = match anki_api::add_note(&format.deck, &format.model, &fields, &tags).await {
        Ok(response) => response,
        Err(e) if e.is_connect() => {
            let error = Failure::new("Anki connection", FailureScope::Shared, e);
            if let Some((record, index)) = &mut batch {
                batches::checkpoint(record, *index, Outcome::Failed { error: error.clone() })?;
            }
            return Err(error);
        }
        Err(e) => {
            return Err(Failure::new(
                "Creating note",
                FailureScope::Stop,
                format!(
                    "Anki did not confirm whether the note was created. Check Anki before mining \
                     it again. {e}"
                ),
            ))
        }
    };
    let note_id = match response.error {
        None => response.result,
        Some(err) if err.contains("duplicate") => {
            if let Some((record, index)) = &mut batch {
                batches::checkpoint(record, *index, Outcome::Duplicate)?;
            }
            return Ok(MineResultDto {
                status: "duplicate".to_string(),
                via,
                warning: None,
                note_id: None,
                media_missing: false,
                key,
            });
        }
        Some(err) => {
            let lower = err.to_lowercase();
            let scope = if ["deck", "model", "note type", "api key", "permission"]
                .iter()
                .any(|s| lower.contains(s))
            {
                FailureScope::Shared
            } else {
                FailureScope::Unknown
            };
            let error = Failure::new("Creating note", scope, err);
            if let Some((record, index)) = &mut batch {
                batches::checkpoint(record, *index, Outcome::Failed { error: error.clone() })?;
            }
            return Err(error);
        }
    };
    let id = note_id.ok_or_else(|| {
        Failure::new(
            "Creating note",
            FailureScope::Stop,
            "Anki returned no note ID. Check Anki before retrying.",
        )
    })?;
    if let Some((record, index)) = &mut batch {
        batches::checkpoint(
            record,
            *index,
            Outcome::Created {
                note_id: id,
                media: match (item.mine_media, options.record) {
                    (false, _) => MediaState::NotRequested,
                    (true, true) => MediaState::Pending,
                    (true, false) => MediaState::Skipped,
                },
                error: None,
            },
        )?;
    }
    if let Some(id) = note_id {
        mined::record_mined_sentence(id, &sentence);
    }

    Ok(MineResultDto {
        status: "created".to_string(),
        via,
        warning: None,
        note_id,
        media_missing: false,
        key,
    })
}

/// Re-run asbplayer enrichment on a note whose media never landed.
#[tauri::command]
pub async fn retry_mine_media(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    note_id: u64,
    timestamp_secs: Option<f32>,
    timestamp_end_secs: Option<f32>,
    timestamp_label: Option<String>,
    progress: Channel<LoadingMessage>,
) -> Result<(), String> {
    let _operation =
        batches::OPERATION.try_lock().map_err(|_| "Mining or undo is already running")?;
    let media_id = { state.lock().unwrap().file.asbplayer_media_id.clone() };
    if target_lacks_subtitles(&player, media_id.as_deref()).await {
        return Err("asbplayer still has no subtitles loaded on the loaded video".to_string());
    }
    let record_secs = cue_duration_secs(timestamp_secs, timestamp_end_secs);
    enrich_and_verify(
        &player,
        note_id,
        media_id,
        timestamp_secs,
        timestamp_label,
        record_secs,
        &progress,
    )
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// asbplayer's `mine-subtitle` drops targets without loaded subtitles, so
/// enriching against one can only fail — detect it up front. Unknown states
/// (no target id, pre-v1.20 extension) fall through to the normal attempt.
async fn target_lacks_subtitles(player: &PlayerHandle, media_id: Option<&str>) -> bool {
    let Some(id) = media_id else { return false };
    let Ok(media) = player.get_bound_media().await else { return false };
    !media.iter().any(|m| m.id == id && !m.loaded_subtitles.is_empty())
}

async fn record_item(
    player: &PlayerHandle,
    note_id: u64,
    media_id: Option<String>,
    item: &BatchItem,
    progress: &Channel<LoadingMessage>,
) -> Result<Option<String>, EnrichError> {
    let start = item.timestamp.as_ref().map(|t| t.start_secs);
    let end = item.timestamp.as_ref().map(|t| t.end_secs);
    let label = item.timestamp.as_ref().map(|t| t.start_label.clone());
    enrich_and_verify(
        player,
        note_id,
        media_id,
        start,
        label,
        cue_duration_secs(start, end),
        progress,
    )
    .await
}

fn cue_duration_secs(start: Option<f32>, end: Option<f32>) -> f32 {
    match (start, end) {
        (Some(s), Some(e)) => (e - s).max(0.0),
        _ => 0.0,
    }
}

enum EnrichError {
    Unverified,
    Failed(String),
}

impl From<String> for EnrichError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

impl std::fmt::Display for EnrichError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => f.write_str(
                "asbplayer didn't update the card. In a new tab, audio recording usually has to be \
                 enabled first: open the video tab, click the asbplayer button in the browser \
                 toolbar and allow recording, then retry. If recording is already enabled, check \
                 asbplayer's Anki settings (deck, note type, and field mappings).",
            ),
            Self::Failed(error) => f.write_str(error),
        }
    }
}

impl EnrichError {
    fn failure(&self) -> Failure {
        let failure = Failure::new("Recording media", FailureScope::Unknown, self);
        match self {
            Self::Unverified => failure.with_kind(FailureKind::MediaUnverified),
            Self::Failed(_) => failure,
        }
    }
}

/// The note's current field values, or `None` when AnkiConnect can't serve it.
async fn snapshot_fields(note_id: u64) -> Option<std::collections::HashMap<String, String>> {
    let notes = anki_api::get_notes(vec![note_id]).await.ok()?;
    let note = notes.into_iter().next()?;
    Some(note.fields.into_iter().map(|(name, field)| (name, field.value)).collect())
}

/// Seek, mine, then verify the enrichment actually changed the note: asbplayer's
/// `published: true` only means the command was broadcast — recording and the
/// note update happen asynchronously afterwards. Verification also catches a
/// pre-v1.20 extension ignoring `noteId` and updating the last-added note.
async fn enrich_and_verify(
    player: &PlayerHandle,
    note_id: u64,
    media_id: Option<String>,
    timestamp_secs: Option<f32>,
    timestamp_label: Option<String>,
    record_secs: f32,
    progress: &Channel<LoadingMessage>,
) -> Result<Option<String>, EnrichError> {
    let _ = progress.send(LoadingMessage::new("Adding audio & screenshot via asbplayer…"));
    let baseline = snapshot_fields(note_id).await;

    if let Some(secs) = timestamp_secs {
        player.seek(secs, timestamp_label.unwrap_or_default(), media_id.clone()).await?;
        wait_for_seek_confirmation(player, secs).await;
    }
    player.mine_subtitle(std::collections::HashMap::new(), 2, media_id, Some(note_id)).await?;

    let Some(baseline) = baseline else {
        return Err(EnrichError::Failed(
            "Recording was requested, but Anki could not be read to verify the media".into(),
        ));
    };

    let _ = progress.send(LoadingMessage::new("Waiting for asbplayer to record the cue…"));
    tokio::time::sleep(Duration::from_secs_f32(record_secs) + RECORD_BUFFER).await;

    let _ = progress.send(LoadingMessage::new("Verifying the media landed in Anki…"));
    let deadline = std::time::Instant::now() + MEDIA_VERIFY_TIMEOUT;
    loop {
        if let Some(now) = snapshot_fields(note_id).await.filter(|now| *now != baseline) {
            return Ok(new_image(&baseline, &now));
        }
        if std::time::Instant::now() >= deadline {
            return Err(EnrichError::Unverified);
        }
        tokio::time::sleep(MEDIA_VERIFY_POLL).await;
    }
}

fn new_image(
    before: &std::collections::HashMap<String, String>,
    after: &std::collections::HashMap<String, String>,
) -> Option<String> {
    after.iter().find_map(|(field, value)| {
        let old = before.get(field).map(String::as_str).unwrap_or_default();
        image_sources(value).into_iter().find(|src| !old.contains(src)).map(str::to_string)
    })
}

fn image_sources(html: &str) -> Vec<&str> {
    html.split("<img")
        .skip(1)
        .filter_map(|tag| {
            let src = tag.split('>').next()?.split_once("src=")?.1;
            match src.chars().next()? {
                quote @ ('"' | '\'') => src[1..].split(quote).next(),
                _ => src.split(char::is_whitespace).next(),
            }
        })
        .filter(|src| !src.is_empty())
        .collect()
}

#[tauri::command]
pub async fn get_media_preview(filename: String) -> Result<Option<String>, String> {
    let extension = filename.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    let mime = match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("avif") => "image/avif",
        _ => return Ok(None),
    };
    let data = anki_api::retrieve_media_file(&filename).await.map_err(|e| e.to_string())?;
    Ok(data.map(|d| format!("data:{mime};base64,{d}")))
}

/// Open Anki's browser on recent adds with the mined note's card selected.
#[tauri::command]
pub async fn open_in_anki(note_id: u64) -> Result<(), String> {
    let response = anki_api::gui_browse(&format!("added:1 OR nid:{}", note_id))
        .await
        .map_err(|e| format!("AnkiConnect is unreachable: {}", e))?;
    if let Some(err) = response.error {
        return Err(err);
    }
    if let Ok(notes) = anki_api::get_notes(vec![note_id]).await {
        if let Some(card) = notes.first().and_then(|n| n.cards.first()) {
            let _ = anki_api::gui_select_card(*card).await;
        }
    }
    Ok(())
}

/// Open Anki's browser on a set of notes (post-batch review).
#[tauri::command]
pub async fn open_notes_in_anki(note_ids: Vec<u64>) -> Result<(), String> {
    let ids = note_ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
    let response = anki_api::gui_browse(&format!("nid:{}", ids))
        .await
        .map_err(|e| format!("AnkiConnect is unreachable: {}", e))?;
    match response.error {
        None => Ok(()),
        Some(err) => Err(err),
    }
}

/// Best-effort seek-ack wait so asbplayer records the right cue; proceeds on
/// timeout rather than failing.
async fn wait_for_seek_confirmation(player: &PlayerHandle, secs: f32) {
    let deadline = std::time::Instant::now() + SEEK_CONFIRM_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if let Ok(status) = player.status().await {
            if status.confirmed_timestamps.iter().any(|t| (t - secs).abs() < 0.01) {
                return;
            }
        }
        tokio::time::sleep(SEEK_CONFIRM_POLL).await;
    }
}

/// Mined/added state for the table (issue #3). Best-effort: an offline
/// AnkiConnect still returns the cached sentences.
#[tauri::command]
pub async fn get_mined_state(state: State<'_, Mutex<AppState>>) -> Result<MinedStateDto, String> {
    let mappings = { state.lock().unwrap().settings.anki_model_mappings.clone() };
    let (added_terms, added_keys, added_sentences) =
        mined::get_recently_added(&mappings).await.unwrap_or_default();

    let mut mined_sentences = mined::mined_sentences_pruned().await;
    mined_sentences.extend(added_sentences);
    Ok(MinedStateDto { added_terms, added_keys, mined_sentences })
}

/// The user's Yomitan term card formats, for the popover's per-format buttons.
#[tauri::command]
pub async fn get_card_formats(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<CardFormatDto>, String> {
    let yomitan_url = { state.lock().unwrap().settings.yomitan_url.clone() };
    Ok(yomitan::get_term_card_formats(&yomitan_url)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| CardFormatDto { name: f.name, deck: f.deck, model: f.model })
        .collect())
}

const DEFINITION_MAX_ENTRIES: u32 = 8;

/// Rendered dictionary entries for the definition popover (issue #113).
/// Empty result = Yomitan has no entry (not an error).
#[tauri::command]
pub async fn render_definition(
    state: State<'_, Mutex<AppState>>,
    term: String,
) -> Result<Vec<DefinitionEntryDto>, String> {
    let yomitan_url = { state.lock().unwrap().settings.yomitan_url.clone() };
    let markers: Vec<String> = ["expression", "reading", "furigana", "frequencies", "glossary"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rendered =
        yomitan::render_fields(&yomitan_url, &term, &markers, DEFINITION_MAX_ENTRIES, false)
            .await
            .map_err(|e| e.to_string())?;

    let mut guard = state.lock().unwrap();
    let known = &*guard.known_entry_keys.get_or_insert_with(mined::known_entry_keys);

    Ok(rendered
        .fields
        .into_iter()
        .enumerate()
        .filter_map(|(index, mut entry)| {
            let glossary_html = entry.remove("glossary").unwrap_or_default();
            if glossary_html.trim().is_empty() {
                return None;
            }
            let expression = entry.remove("expression").unwrap_or_default();
            let reading = entry.remove("reading").unwrap_or_default();
            let key = mined::entry_key(&expression, &reading);
            Some(DefinitionEntryDto {
                index,
                known: known.contains(&key),
                key,
                expression,
                reading,
                furigana_html: entry.remove("furigana").unwrap_or_default(),
                frequencies_html: entry.remove("frequencies").unwrap_or_default(),
                glossary_html,
            })
        })
        .collect())
}

/// Reachability probe; `url` lets the modal test a staged (unsaved) value.
#[tauri::command]
pub async fn get_yomitan_status(
    state: State<'_, Mutex<AppState>>,
    url: Option<String>,
) -> Result<YomitanStatusDto, String> {
    let yomitan_url = url.unwrap_or_else(|| state.lock().unwrap().settings.yomitan_url.clone());
    match yomitan::get_version(&yomitan_url).await {
        Ok(version) => Ok(YomitanStatusDto { reachable: true, version: Some(version) }),
        Err(_) => Ok(YomitanStatusDto { reachable: false, version: None }),
    }
}
