//! One-click mining (issue #105) + mined-state tracking (issue #3). The note
//! is always created via AnkiConnect from Yomitan-rendered fields; the
//! asbplayer path then enriches it (audio/screenshot) via a note-targeted
//! `mine-subtitle` update.

use std::{
    collections::HashMap,
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
        FieldMapping,
    },
    core::settings::MiningMode,
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
        TimeStampDto,
        YomitanStatusDto,
    },
    events::LoadingMessage,
    media::{
        media_fields,
        MediaSource,
    },
    player_task::PlayerHandle,
    state::AppState,
};

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
    let item_key = reading.map(|r| format!("{term} {r}")).unwrap_or_default();
    let (yomitan_url, media, local_mappings, lexeme, source) = {
        let guard = state.lock().unwrap();
        (
            guard.settings.yomitan_url.clone(),
            MediaSource::for_file(&guard, None),
            local_mappings(&guard),
            row_lexeme(&guard.file, &item_key),
            BatchSource::from_file(&guard.file).ok(),
        )
    };
    let item = BatchItem {
        key: item_key,
        lemma: term,
        surface,
        sentence,
        timestamp: timestamp(timestamp_secs, timestamp_end_secs, timestamp_label),
        entry_index,
        format_name,
        scan_text: None,
        adhoc: false,
        mine_media: via != "direct",
        outcome: Outcome::Unattempted,
    };
    let options = MineOptions { record: true, require_dictionary_media: true };
    let mut result = mine(
        &yomitan_url,
        &item,
        lexeme,
        options,
        &progress,
        None,
        source.as_ref(),
        local_mappings.as_ref(),
    )
    .await
    .map_err(|e| e.message)?;

    // Enrichment failures don't undo the mine (the note exists) — warn instead.
    if let Some(id) = result.note_id.filter(|_| item.mine_media) {
        let warning = match media {
            Err(reason) => Some(format!("Card created without audio and a screenshot. {reason}.")),
            Ok(media) if media.lacks_subtitles(&player).await => Some(
                "asbplayer has no subtitles loaded on the loaded video — card created without \
                 audio/screenshot"
                    .to_string(),
            ),
            Ok(media) => media
                .attach(&player, id, item.timestamp.as_ref(), &progress)
                .await
                .err()
                .map(|e| format!("Card created, but media wasn't added: {e}")),
        };
        result.media_missing = warning.is_some();
        result.warning = warning;
    }
    Ok(result)
}

/// The mappings local mining checks before creating a note; `None` in asbplayer mode.
fn local_mappings(state: &AppState) -> Option<HashMap<String, FieldMapping>> {
    (state.settings.mining_mode == MiningMode::Local)
        .then(|| state.settings.anki_model_mappings.clone())
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
    batches::require_profile(&batch, "continue it").await?;
    let item = batch.items.get(item_index).ok_or("Batch item not found")?.clone();
    let (url, media, local_mappings, lexeme) = {
        let state = state.lock().unwrap();
        if !batch.source.matches(&BatchSource::from_file(&state.file)?) {
            return Err("Load the original source before retrying this batch".into());
        }
        (
            state.settings.yomitan_url.clone(),
            MediaSource::for_file(&state, media_target),
            local_mappings(&state).filter(|_| item.mine_media && options.record),
            (!item.adhoc).then(|| row_lexeme(&state.file, &item.key)).flatten(),
        )
    };
    let mut preview_file = None;
    let result = async {
        match item.outcome {
            Outcome::Unattempted | Outcome::Failed { .. } => {
                mine(
                    &url,
                    &item,
                    lexeme,
                    options,
                    &progress,
                    Some((&mut batch, item_index)),
                    None,
                    local_mappings.as_ref(),
                )
                .await?;
            }
            Outcome::Created {
                note_id,
                media: MediaState::Pending | MediaState::Failed | MediaState::Skipped,
                ..
            } => {
                let media = media
                    .as_ref()
                    .map_err(|reason| Failure::new("Local video", FailureScope::Shared, reason))?;
                media.validate(&player).await?;
                batches::checkpoint(
                    &mut batch,
                    item_index,
                    Outcome::Created { note_id, media: MediaState::Pending, error: None },
                )?;
                let result =
                    media.attach(&player, note_id, item.timestamp.as_ref(), &progress).await;
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

/// UniDic's lexeme for a row, keyed by `termKey`: "{lemma} {reading}".
fn row_lexeme(file: &crate::state::FileData, key: &str) -> Option<String> {
    let (lemma, reading) = key.split_once(' ')?;
    file.base_terms
        .iter()
        .find(|t| t.lemma_form == lemma && t.lemma_reading == reading)?
        .lexeme
        .clone()
}

/// Yomitan's first entry can be a different word with the same spelling (止める as やめる when
/// the sentence reads とめる), so a row picks the entry for its own word.
async fn default_entry(
    yomitan_url: &str,
    item: &BatchItem,
    term: &str,
    lexeme: Option<&str>,
) -> usize {
    // A row's key is `termKey`: "{lemma} {reading}".
    let reading = match item.key.split_once(' ') {
        Some((lemma, reading)) if !item.adhoc && lemma == item.lemma => reading,
        _ => return 0,
    };
    tokio::time::timeout(
        MATCH_LOOKUP_TIMEOUT,
        yomitan::entry_index_for(
            yomitan_url,
            term,
            &item.lemma,
            reading,
            lexeme.map(yomine::segmentation::word::lexeme_name),
        ),
    )
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
async fn mine(
    yomitan_url: &str,
    item: &BatchItem,
    lexeme: Option<String>,
    options: MineOptions,
    progress: &Channel<LoadingMessage>,
    mut batch: Option<(&mut BatchRecord, usize)>,
    // The loaded source of a single mine; a batch carries its own.
    source: Option<&BatchSource>,
    local_mappings: Option<&HashMap<String, FieldMapping>>,
) -> Result<MineResultDto, Failure> {
    let term = item.scan_text.as_ref().unwrap_or(&item.lemma).clone();
    let surface = item.surface.clone();
    let sentence = item.sentence.clone();
    let entry_index = item.entry_index;
    let format_name = item.format_name.clone();
    let timestamp_secs = item.timestamp.as_ref().map(|t| t.start_secs);
    let via = if item.mine_media { "media" } else { "direct" }.to_string();
    let entry_index = match entry_index {
        Some(index) => index,
        None => default_entry(yomitan_url, item, &term, lexeme.as_deref()).await,
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
    // Before the note exists, so a missing mapping doesn't leave cards without media.
    if let Some(mappings) = local_mappings.filter(|_| item.mine_media) {
        if media_fields(mappings.get(&format.model)).is_none() {
            let mut failure = Failure::new(
                "Media fields",
                FailureScope::Shared,
                format!(
                    "Choose where {} keeps sentence audio and screenshots in Anki Settings, then \
                     mine again.",
                    format.model
                ),
            )
            .with_kind(FailureKind::MediaFieldsUnset);
            failure.note_type = Some(format.model.clone());
            return Err(failure);
        }
    }
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
    if batch.as_ref().is_some_and(|(record, _)| record.auto) {
        tags.push("yomine::auto".to_string());
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
        let (source, batch_id) = match &batch {
            Some((record, _)) => (Some(&record.source), Some(record.id.as_str())),
            None => (source, None),
        };
        let collection = yomine::anki::api::active_profile().await.ok();
        mined::record_note(
            id,
            &sentence,
            &item.lemma,
            source.map(|s| s.fingerprint.as_str()),
            batch_id,
            collection.as_deref(),
        );
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
    let media = { MediaSource::for_file(&state.lock().unwrap(), None)? };
    if media.lacks_subtitles(&player).await {
        return Err("asbplayer still has no subtitles loaded on the loaded video".to_string());
    }
    let timestamp = timestamp(timestamp_secs, timestamp_end_secs, timestamp_label);
    media
        .attach(&player, note_id, timestamp.as_ref(), &progress)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn timestamp(
    start_secs: Option<f32>,
    end_secs: Option<f32>,
    label: Option<String>,
) -> Option<TimeStampDto> {
    start_secs.map(|start_secs| TimeStampDto {
        start_secs,
        end_secs: end_secs.unwrap_or(start_secs),
        start_label: label.unwrap_or_default(),
        end_label: String::new(),
    })
}

/// A file from Anki's media folder as a data URI; `None` for a type the webview can't show
/// or play.
#[tauri::command]
pub async fn get_media_preview(filename: String) -> Result<Option<String>, String> {
    let extension = filename.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase());
    let mime = match extension.as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("avif") => "image/avif",
        Some("mp3") => "audio/mpeg",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("m4a" | "aac") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("webm") => "audio/webm",
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
