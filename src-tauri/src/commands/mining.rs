//! One-click mining (issue #105) + mined-state tracking (issue #3). The note
//! is always created in Anki from Yomitan-rendered fields; the
//! asbplayer path then enriches it (audio/screenshot) via a note-targeted
//! `mine-subtitle` update.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{
        Duration,
        Instant,
    },
};

use tauri::{
    ipc::Channel,
    State,
};
use yomine::{
    anki::{
        self,
        mined,
        AnkiError,
        CreateOutcome,
        FieldMapping,
        NewNote,
    },
    core::{
        settings::MiningMode,
        YomineError,
    },
    yomitan::{
        self,
        EntryChoice,
    },
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
        MinedStateDto,
        TimeStampDto,
        YomitanStatusDto,
    },
    events::LoadingMessage,
    media::{
        media_fields,
        LineMedia,
        MediaSource,
        Preview,
    },
    player_task::PlayerHandle,
    state::AppState,
};

/// Cloze refinement is optional; this caps how long it can delay a mine.
const MATCH_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

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
    preview_image: Option<String>,
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
    let (mut preview_file, mut preview_image) = (None, None);
    let result = async {
        match item.outcome {
            Outcome::Unattempted | Outcome::Failed { .. } => {
                mine(
                    &url,
                    &item,
                    lexeme,
                    options,
                    &progress,
                    &mut batch,
                    item_index,
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
                    Ok(preview) => {
                        match preview {
                            Some(Preview::AnkiFile(file)) => preview_file = Some(file),
                            Some(Preview::DataUri(uri)) => preview_image = Some(uri),
                            None => {}
                        }
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
            Outcome::Attempting { ref key } => {
                let anki = anki::current();
                match key.as_deref().and_then(unconfirmed) {
                    Some(request) if anki.can_resend_create() => {
                        create(&anki, request, &item, options, &mut batch, item_index).await?
                    }
                    _ => return Err(unconfirmed_stop(None)),
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
    Ok(BatchStep { batch, failure, preview_file, preview_image })
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

/// A render request that failed outright. It pauses the batch only when Yomitan has stopped
/// answering; otherwise the item is worth trying again later.
async fn request_failure(yomitan_url: &str, term: &str, error: YomineError) -> Failure {
    if let Err(e) = yomitan::get_version(yomitan_url).await {
        return Failure::new("Yomitan connection", FailureScope::Shared, e);
    }
    let message = if matches!(&error, YomineError::Reqwest(e) if e.is_timeout()) {
        format!("Yomitan didn't finish rendering 「{term}」 in time")
    } else {
        format!("Yomitan dropped the request for 「{term}」: {error}")
    };
    Failure::new("Rendering card", FailureScope::Unknown, message).with_kind(FailureKind::Transient)
}

/// Cuts these lines' media in the background, for review frames and a batch's record step.
#[tauri::command]
pub fn prepare_media(
    state: State<'_, Mutex<AppState>>,
    cues: Vec<TimeStampDto>,
) -> Result<(), String> {
    if let Ok(source) = MediaSource::for_file(&state.lock().unwrap(), None) {
        crate::media::prepare(&source, cues);
    }
    Ok(())
}

/// `None` without a paired video or in asbplayer mode.
#[tauri::command]
pub async fn get_line_media(
    state: State<'_, Mutex<AppState>>,
    cue: TimeStampDto,
    kind: LineMedia,
) -> Result<Option<String>, String> {
    let Ok(source) = MediaSource::for_file(&state.lock().unwrap(), None) else { return Ok(None) };
    crate::media::line_media(&source, &cue, kind).await
}

/// The entry mining picks when none was chosen, as an index into the scan of `scan_text`.
/// `None` when no entry matches the whole term.
#[tauri::command]
pub async fn get_default_entry(
    state: State<'_, Mutex<AppState>>,
    key: String,
    lemma: String,
    scan_text: Option<String>,
) -> Result<Option<usize>, String> {
    let (url, lexeme) = {
        let state = state.lock().unwrap();
        (state.settings.yomitan_url.clone(), row_lexeme(&state.file, &key))
    };
    let term = scan_text.unwrap_or_else(|| lemma.clone());
    Ok(default_entry(&url, &key, &lemma, &term, lexeme.as_deref()).await)
}

/// Yomitan's first entry can be a different word with the same spelling (止める as やめる when
/// the sentence reads とめる), so a row picks the entry for its own word. `None` when no entry
/// matches the whole term.
async fn default_entry(
    yomitan_url: &str,
    key: &str,
    lemma: &str,
    term: &str,
    lexeme: Option<&str>,
) -> Option<usize> {
    // A row's key is `termKey`: "{lemma} {reading}".
    let reading = match key.split_once(' ') {
        Some((key_lemma, reading)) if key_lemma == lemma => reading,
        _ => return Some(0),
    };
    let choice = tokio::time::timeout(
        MATCH_LOOKUP_TIMEOUT,
        yomitan::entry_index_for(
            yomitan_url,
            term,
            lemma,
            reading,
            lexeme.map(yomine::segmentation::word::lexeme_name),
        ),
    )
    .await
    .ok()
    .flatten();
    match choice {
        Some(EntryChoice::PartOnly) => None,
        Some(EntryChoice::Entry(index)) => Some(index),
        None => Some(0),
    }
}

#[allow(clippy::too_many_arguments)]
async fn mine(
    yomitan_url: &str,
    item: &BatchItem,
    lexeme: Option<String>,
    options: MineOptions,
    progress: &Channel<LoadingMessage>,
    batch: &mut BatchRecord,
    index: usize,
    local_mappings: Option<&HashMap<String, FieldMapping>>,
) -> Result<(), Failure> {
    let term = item.scan_text.as_ref().unwrap_or(&item.lemma).clone();
    let surface = item.surface.clone();
    let sentence = item.sentence.clone();
    let entry_index = item.entry_index;
    let format_name = item.format_name.clone();
    let timestamp_secs = item.timestamp.as_ref().map(|t| t.start_secs);
    let entry_index = match entry_index {
        Some(index) => index,
        None if item.adhoc => 0,
        None => default_entry(yomitan_url, &item.key, &item.lemma, &term, lexeme.as_deref())
            .await
            .ok_or_else(|| {
                Failure::new(
                    "Dictionary entry",
                    FailureScope::Item,
                    format!("Yomitan has no dictionary entry for all of 「{}」", term),
                )
                .with_kind(FailureKind::PartOnly)
            })?,
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
        match yomitan::render_fields(yomitan_url, &term, &markers, entry_index as u32 + 1, true)
            .await
        {
            Ok(rendered) => rendered,
            Err(e @ YomineError::Reqwest(_)) => {
                return Err(request_failure(yomitan_url, &term, e).await)
            }
            Err(e) => return Err(Failure::new("Rendering card", FailureScope::Unknown, e)),
        };

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
    let mut fields = yomitan::assemble_fields(format, marker_values, Some(ctx));
    if fields.is_empty() {
        return Err(Failure::new(
            "Rendering card",
            FailureScope::Item,
            format!("Yomitan rendered no card content for 「{}」", term),
        ));
    }

    let _ = progress.send(LoadingMessage::new("Creating Anki note…"));

    let anki = anki::current();
    for media in rendered.audio_media.iter().chain(rendered.dictionary_media.iter()) {
        match anki.store_media(&media.anki_filename, &media.content).await {
            // Anki renames a file whose name is taken by different bytes.
            Ok(stored) if stored != media.anki_filename => {
                for value in fields.values_mut() {
                    *value = value.replace(&media.anki_filename, &stored);
                }
            }
            Ok(_) => {}
            Err(AnkiError::Rejected(error)) if options.require_dictionary_media => {
                return Err(Failure::new(
                    "Uploading media",
                    FailureScope::Unknown,
                    format!("{}: {}", media.anki_filename, error),
                )
                .with_fallback(Fallback::WithoutDictionaryMedia))
            }
            Err(AnkiError::Rejected(error)) => {
                eprintln!("storeMediaFile {}: {}", media.anki_filename, error)
            }
            Err(e) => return Err(Failure::new("Uploading media", FailureScope::Shared, e)),
        }
    }

    // No timestamp = no attachable media (EPUB/TXT); tags the note for a future re-mine flow.
    let mut tags = vec!["yomine".to_string()];
    if timestamp_secs.is_none() {
        tags.push("yomine::no-media".to_string());
    }
    if batch.auto {
        tags.push("yomine::auto".to_string());
    }
    let note = NewNote { deck: format.deck.clone(), note_type: format.model.clone(), fields, tags };
    let request = CreateRequest { key: anki::request_key(), note, first_sent: Instant::now() };
    create(&anki, request, item, options, batch, index).await
}

/// A create request as sent, kept to resend unchanged: Tsunagi refuses a reused key with
/// a different body, and a new render can differ.
struct CreateRequest {
    key: String,
    note: NewNote,
    first_sent: Instant,
}

/// Tsunagi keeps a key for ten minutes; a later resend runs as a new request.
const RESEND_WINDOW: Duration = Duration::from_secs(9 * 60);

static UNCONFIRMED: Mutex<Option<CreateRequest>> = Mutex::new(None);

const NOT_CONFIRMED: &str = "Anki did not confirm whether the note was created.";

/// The unconfirmed request with `key`, while resending it is still safe.
fn unconfirmed(key: &str) -> Option<CreateRequest> {
    let request = UNCONFIRMED.lock().unwrap().take_if(|r| r.key == key)?;
    (request.first_sent.elapsed() < RESEND_WINDOW).then_some(request)
}

fn unconfirmed_stop(error: Option<AnkiError>) -> Failure {
    let check = format!("{NOT_CONFIRMED} Check Anki before mining it again.");
    let message = match error {
        Some(e) => format!("{check} {e}"),
        None => check,
    };
    Failure::new("Creating note", FailureScope::Stop, message)
}

async fn create(
    anki: &anki::Anki,
    request: CreateRequest,
    item: &BatchItem,
    options: MineOptions,
    batch: &mut BatchRecord,
    index: usize,
) -> Result<(), Failure> {
    batches::checkpoint(batch, index, Outcome::Attempting { key: Some(request.key.clone()) })?;
    let id = match anki.create_note(&request.note, &request.key).await {
        Ok(CreateOutcome::Created(id)) => id,
        Ok(CreateOutcome::Duplicate) => {
            return batches::checkpoint(batch, index, Outcome::Duplicate)
        }
        Ok(CreateOutcome::Rejected { reason, setup }) => {
            let scope = if setup { FailureScope::Shared } else { FailureScope::Unknown };
            let error = Failure::new("Creating note", scope, reason);
            batches::checkpoint(batch, index, Outcome::Failed { error: error.clone() })?;
            return Err(error);
        }
        Err(e @ AnkiError::NotSent(_)) => {
            let error = Failure::new("Anki connection", FailureScope::Shared, e);
            batches::checkpoint(batch, index, Outcome::Failed { error: error.clone() })?;
            return Err(error);
        }
        Err(e) if anki.can_resend_create() => {
            let message =
                format!("{NOT_CONFIRMED} Retrying asks again without adding it twice. {e}");
            *UNCONFIRMED.lock().unwrap() = Some(request);
            return Err(Failure::new("Creating note", FailureScope::Unknown, message));
        }
        Err(e) => return Err(unconfirmed_stop(Some(e))),
    };
    batches::checkpoint(
        batch,
        index,
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
    let collection = anki.profile().await.ok();
    mined::record_note(
        id,
        &item.sentence,
        &item.lemma,
        Some(batch.source.fingerprint.as_str()),
        Some(batch.id.as_str()),
        collection.as_deref(),
    );
    Ok(())
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
    let data = anki::current().media(&filename).await.map_err(|e| e.to_string())?;
    Ok(data.map(|d| format!("data:{mime};base64,{d}")))
}

/// Open Anki's browser on recent adds with the mined note's card selected.
#[tauri::command]
pub async fn open_in_anki(note_id: u64) -> Result<(), String> {
    anki::current().open_note(note_id).await.map_err(gui_error)
}

/// Open Anki's browser on a set of notes (post-batch review).
#[tauri::command]
pub async fn open_notes_in_anki(note_ids: Vec<u64>) -> Result<(), String> {
    anki::current().browse_notes(&note_ids).await.map_err(gui_error)
}

fn gui_error(error: AnkiError) -> String {
    match error {
        AnkiError::Rejected(message) => message,
        e => format!("Anki is unreachable: {e}"),
    }
}

/// Mined state for the table (issue #3), from Yomine's copy of the collection; later
/// changes arrive as `mined-state` events.
#[tauri::command]
pub fn get_mined_state(state: State<'_, Mutex<AppState>>) -> MinedStateDto {
    crate::anki_sync::mined_state(&state.lock().unwrap())
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
