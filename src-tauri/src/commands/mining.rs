//! One-click mining (issue #105) + mined-state tracking (issue #3). The note
//! is always created via AnkiConnect from Yomitan-rendered fields; the
//! asbplayer path then enriches it (audio/screenshot) via "update last card".

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
    dto::{
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

#[tauri::command]
pub async fn mine_term(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    term: String,
    sentence: String,
    timestamp_secs: Option<f32>,
    timestamp_end_secs: Option<f32>,
    timestamp_label: Option<String>,
    via: String,
    progress: Channel<LoadingMessage>,
) -> Result<MineResultDto, String> {
    let yomitan_url = { state.lock().unwrap().settings.yomitan_url.clone() };

    let _ = progress.send(LoadingMessage::new(format!("Rendering 「{}」 with Yomitan…", term)));
    let format = yomitan::get_term_card_format(&yomitan_url).await.map_err(|e| e.to_string())?;
    let markers = yomitan::collect_markers(&format);
    let rendered = yomitan::render_fields(&yomitan_url, &term, &markers, true)
        .await
        .map_err(|e| e.to_string())?;

    let empty = std::collections::HashMap::new();
    let marker_values = rendered.fields.first().unwrap_or(&empty);
    if marker_values.values().all(|v| v.trim().is_empty()) {
        return Err(format!("Yomitan has no dictionary entry for 「{}」", term));
    }

    let ctx = yomitan::SentenceContext { sentence: &sentence, term: &term };
    let fields = yomitan::assemble_fields(&format, marker_values, Some(ctx));
    if fields.is_empty() {
        return Err(format!("Yomitan rendered no card content for 「{}」", term));
    }

    let _ = progress.send(LoadingMessage::new("Creating Anki note…"));

    // Media failures degrade the note (missing audio/image), not the mine.
    for media in rendered.audio_media.iter().chain(rendered.dictionary_media.iter()) {
        match anki_api::store_media_file(&media.anki_filename, &media.content).await {
            Ok(response) if response.error.is_none() => {}
            Ok(response) => {
                eprintln!("storeMediaFile {}: {:?}", media.anki_filename, response.error)
            }
            Err(e) => eprintln!("storeMediaFile {}: {}", media.anki_filename, e),
        }
    }

    let response =
        anki_api::add_note(&format.deck, &format.model, &fields, &["yomine".to_string()])
            .await
            .map_err(|e| format!("AnkiConnect is unreachable: {}", e))?;
    let note_id = match response.error {
        None => response.result,
        Some(err) if err.contains("duplicate") => {
            // No enrichment: "update last card" would hit an unrelated note.
            return Ok(MineResultDto {
                status: "duplicate".to_string(),
                via,
                warning: None,
                note_id: None,
                media_missing: false,
            });
        }
        Some(err) => return Err(err),
    };
    if let Some(id) = note_id {
        mined::record_mined_sentence(id, &sentence);
    }

    // Enrichment failures don't undo the mine (the note exists) — warn instead.
    let mut warning = None;
    let mut media_missing = false;
    if via == "asbplayer" {
        if let Some(id) = note_id {
            let record_secs = cue_duration_secs(timestamp_secs, timestamp_end_secs);
            if let Err(e) = enrich_and_verify(
                &player,
                id,
                timestamp_secs,
                timestamp_label,
                record_secs,
                &progress,
            )
            .await
            {
                warning = Some(format!("Card created, but media wasn't added: {}", e));
                media_missing = true;
            }
        }
    }

    Ok(MineResultDto { status: "created".to_string(), via, warning, note_id, media_missing })
}

/// Re-run asbplayer enrichment on a note whose media never landed. Only safe
/// while the note is still Anki's newest — "update last card" has no way to
/// target a specific note.
#[tauri::command]
pub async fn retry_mine_media(
    player: State<'_, PlayerHandle>,
    note_id: u64,
    timestamp_secs: Option<f32>,
    timestamp_end_secs: Option<f32>,
    timestamp_label: Option<String>,
    progress: Channel<LoadingMessage>,
) -> Result<(), String> {
    let recent = anki_api::get_note_ids("added:7")
        .await
        .map_err(|e| format!("AnkiConnect is unreachable: {}", e))?;
    match recent.iter().max() {
        Some(&max) if max == note_id => {}
        Some(&max) if max > note_id => {
            return Err("A newer note was added since — asbplayer can only update the most \
                        recent note. Add the media in Anki instead."
                .to_string());
        }
        _ => return Err("Couldn't confirm the card is still Anki's most recent note.".to_string()),
    }

    let record_secs = cue_duration_secs(timestamp_secs, timestamp_end_secs);
    enrich_and_verify(&player, note_id, timestamp_secs, timestamp_label, record_secs, &progress)
        .await
}

fn cue_duration_secs(start: Option<f32>, end: Option<f32>) -> f32 {
    match (start, end) {
        (Some(s), Some(e)) => (e - s).max(0.0),
        _ => 0.0,
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
/// "update last card" write happen asynchronously afterwards.
async fn enrich_and_verify(
    player: &PlayerHandle,
    note_id: u64,
    timestamp_secs: Option<f32>,
    timestamp_label: Option<String>,
    record_secs: f32,
    progress: &Channel<LoadingMessage>,
) -> Result<(), String> {
    let _ = progress.send(LoadingMessage::new("Adding audio & screenshot via asbplayer…"));
    let baseline = snapshot_fields(note_id).await;

    if let Some(secs) = timestamp_secs {
        player.seek(secs, timestamp_label.unwrap_or_default()).await?;
        wait_for_seek_confirmation(player, secs).await;
    }
    player.mine_subtitle(std::collections::HashMap::new(), 2).await?;

    // AnkiConnect hiccup on the baseline read: enrichment ran, verification can't.
    let Some(baseline) = baseline else { return Ok(()) };

    let _ = progress.send(LoadingMessage::new("Waiting for asbplayer to record the cue…"));
    tokio::time::sleep(Duration::from_secs_f32(record_secs) + RECORD_BUFFER).await;

    let _ = progress.send(LoadingMessage::new("Verifying the media landed in Anki…"));
    let deadline = std::time::Instant::now() + MEDIA_VERIFY_TIMEOUT;
    loop {
        if snapshot_fields(note_id).await.is_some_and(|now| now != baseline) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err("asbplayer accepted the mine but never updated the card — check its \
                        Anki settings (deck, note type, and field mappings)"
                .to_string());
        }
        tokio::time::sleep(MEDIA_VERIFY_POLL).await;
    }
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
    let (added_terms, added_sentences) =
        mined::get_recently_added(&mappings).await.unwrap_or_default();

    let mut mined_sentences = mined::mined_sentences_pruned().await;
    mined_sentences.extend(added_sentences);
    Ok(MinedStateDto { added_terms, mined_sentences })
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
