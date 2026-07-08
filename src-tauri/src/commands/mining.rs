//! One-click mining (issue #105) + mined-state tracking (issue #3).
//!
//! Card content comes from the user's own Yomitan config via yomitan-api
//! (`src/yomitan`): their term card format decides deck/model/fields, their
//! templates render the marker values. The note is always created by Yomine
//! via AnkiConnect; when asbplayer is the active player (`via: "asbplayer"`)
//! we then seek to the cue and send `mine-subtitle` with "update last card",
//! so asbplayer enriches the fresh note with its audio/screenshot — the same
//! order as asbplayer's own one-click-mining architecture.

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

/// How long we poll for the seek confirmation before mining anyway — asbplayer
/// acks confirmed seeks well inside this in practice.
const SEEK_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);
const SEEK_CONFIRM_POLL: Duration = Duration::from_millis(250);

#[tauri::command]
pub async fn mine_term(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    term: String,
    sentence: String,
    timestamp_secs: Option<f32>,
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
            // Don't enrich a duplicate: "update last card" would hit whatever
            // note happens to be newest, not this term's existing card.
            return Ok(MineResultDto {
                status: "duplicate".to_string(),
                via,
                warning: None,
                note_id: None,
            });
        }
        Some(err) => return Err(err),
    };
    if let Some(id) = note_id {
        mined::record_mined_sentence(id, &sentence);
    }

    // asbplayer enrichment: seek to the cue, then "update last card" (2) makes
    // asbplayer attach its audio/screenshot to the note just created. Failures
    // don't undo the mine — the note exists — so they come back as a warning.
    let mut warning = None;
    if via == "asbplayer" {
        let _ = progress.send(LoadingMessage::new("Adding audio & screenshot via asbplayer…"));
        let mut enrich_err: Option<String> = None;
        if let Some(secs) = timestamp_secs {
            match player.seek(secs, timestamp_label.unwrap_or_default()).await {
                Ok(()) => wait_for_seek_confirmation(&player, secs).await,
                Err(e) => enrich_err = Some(e),
            }
        }
        if enrich_err.is_none() {
            if let Err(e) = player.mine_subtitle(std::collections::HashMap::new(), 2).await {
                enrich_err = Some(e);
            }
        }
        warning =
            enrich_err.map(|e| format!("Card created, but asbplayer couldn't enrich it: {}", e));
    }

    Ok(MineResultDto { status: "created".to_string(), via, warning, note_id })
}

/// Open Anki's browser on the mined note (`guiBrowse nid:`).
#[tauri::command]
pub async fn open_in_anki(note_id: u64) -> Result<(), String> {
    let response = anki_api::gui_browse(&format!("nid:{}", note_id))
        .await
        .map_err(|e| format!("AnkiConnect is unreachable: {}", e))?;
    match response.error {
        None => Ok(()),
        Some(err) => Err(err),
    }
}

/// Best-effort wait for asbplayer to ack the seek, so `mine-subtitle` sees the
/// term's subtitle as "current". Proceeds on timeout rather than failing.
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

/// Mined/added state for the table (issue #3): terms with a card added in the
/// last day, plus the harvested + freshly-added sentence sets. Best-effort —
/// an offline AnkiConnect still returns the cached sentences.
#[tauri::command]
pub async fn get_mined_state(state: State<'_, Mutex<AppState>>) -> Result<MinedStateDto, String> {
    let mappings = { state.lock().unwrap().settings.anki_model_mappings.clone() };
    let (added_terms, added_sentences) =
        mined::get_recently_added(&mappings).await.unwrap_or_default();

    let mut mined_sentences = mined::mined_sentences_pruned().await;
    mined_sentences.extend(added_sentences);
    Ok(MinedStateDto { added_terms, mined_sentences })
}

/// Reachability probe for the Anki settings modal. `url` overrides the saved
/// setting so the modal can test a staged (unsaved) value.
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
