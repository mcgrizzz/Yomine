use std::sync::{
    atomic::{
        AtomicBool,
        Ordering,
    },
    Mutex,
};

use serde::{
    Deserialize,
    Serialize,
};
use tauri::State;
use yomine::{
    anki::{
        api,
        mined,
    },
    core::{
        models::SourceFile,
        Sentence,
    },
    persistence::db,
};

use crate::{
    dto::TimeStampDto,
    state::{
        AppState,
        FileData,
    },
};

pub static OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
pub static RUNNING: AtomicBool = AtomicBool::new(false);
pub static AUTO: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct BatchSource {
    pub title: String,
    pub locator: String,
    pub fingerprint: String,
}

impl BatchSource {
    pub fn from_file(file: &FileData) -> Result<Self, String> {
        let source = file.source_file.as_ref().ok_or("Load the original source first")?;
        Ok(Self::new(source, &file.sentences))
    }

    pub fn new(source: &SourceFile, sentences: &[Sentence]) -> Self {
        let sentences: Vec<_> = sentences
            .iter()
            .map(|s| {
                let times = s.timestamp.as_ref().map(|t| {
                    let (a, b) = t.to_secs();
                    ((a * 1000.0).round() as i64, (b * 1000.0).round() as i64)
                });
                (&s.text, times)
            })
            .collect();
        let bytes = serde_json::to_vec(&(source.epub_chapters.as_ref(), sentences))
            .expect("source identity is plain data");
        let hash = bytes
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3));
        Self {
            title: source.title.clone(),
            locator: source.original_file.clone(),
            fingerprint: format!("{hash:016x}"),
        }
    }

    pub fn matches(&self, other: &Self) -> bool {
        self.locator == other.locator && self.fingerprint == other.fingerprint
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FailureScope {
    Item,
    Shared,
    Unknown,
    Stop,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Fallback {
    WithoutDictionaryMedia,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    MediaUnverified,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Failure {
    pub stage: String,
    pub scope: FailureScope,
    pub message: String,
    pub fallback: Option<Fallback>,
    pub kind: Option<FailureKind>,
}

impl Failure {
    pub fn new(stage: &str, scope: FailureScope, message: impl ToString) -> Self {
        Self {
            stage: stage.into(),
            scope,
            message: message.to_string(),
            fallback: None,
            kind: None,
        }
    }
    pub fn with_kind(self, kind: FailureKind) -> Self {
        Self { kind: Some(kind), ..self }
    }
    pub fn with_fallback(self, fallback: Fallback) -> Self {
        Self { fallback: Some(fallback), ..self }
    }
    pub fn storage(message: impl ToString) -> Self {
        Self::new("Saving batch", FailureScope::Stop, message)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MediaState {
    NotRequested,
    Pending,
    Complete,
    Failed,
    Skipped,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    #[default]
    Unattempted,
    Attempting,
    Created {
        note_id: u64,
        media: MediaState,
        error: Option<Failure>,
    },
    Duplicate,
    Failed {
        error: Failure,
    },
    Deleted {
        note_id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BatchItem {
    pub key: String,
    pub lemma: String,
    pub surface: String,
    pub sentence: String,
    pub timestamp: Option<TimeStampDto>,
    pub entry_index: Option<usize>,
    pub format_name: Option<String>,
    pub scan_text: Option<String>,
    pub adhoc: bool,
    pub mine_media: bool,
    #[serde(default)]
    pub outcome: Outcome,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BatchRecord {
    pub id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub source: BatchSource,
    pub items: Vec<BatchItem>,
    #[serde(default)]
    pub auto: bool,
}

impl BatchRecord {
    fn stored(&self) -> db::batches::StoredBatch {
        db::batches::StoredBatch {
            id: self.id.clone(),
            fingerprint: self.source.fingerprint.clone(),
            title: self.source.title.clone(),
            path: self.source.locator.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
            auto: self.auto,
            items: self
                .items
                .iter()
                .map(|item| serde_json::to_string(item).expect("batch items are plain data"))
                .collect(),
        }
    }

    fn from_stored(stored: db::batches::StoredBatch) -> Result<Self, String> {
        let items = stored
            .items
            .iter()
            .map(|item| serde_json::from_str(item))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("The last batch record is unreadable: {e}"))?;
        Ok(Self {
            id: stored.id,
            started_at: stored.started_at,
            finished_at: stored.finished_at,
            source: BatchSource {
                title: stored.title,
                locator: stored.path,
                fingerprint: stored.fingerprint,
            },
            items,
            auto: stored.auto,
        })
    }
}

pub fn load(id: &str) -> Result<BatchRecord, String> {
    get_last_batch()?
        .filter(|b| b.id == id)
        .ok_or_else(|| "This batch is no longer available".into())
}

pub fn save(batch: &BatchRecord) -> Result<(), Failure> {
    match db::with(|conn| db::batches::save_latest(conn, &batch.stored())) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Failure::storage("This batch has been replaced by a newer batch")),
        Err(e) => Err(Failure::storage(e)),
    }
}

pub fn checkpoint(batch: &mut BatchRecord, index: usize, outcome: Outcome) -> Result<(), Failure> {
    batch.items.get_mut(index).ok_or_else(|| Failure::storage("Batch item not found"))?.outcome =
        outcome;
    batch.finished_at = None;
    save(batch)
}

#[tauri::command]
pub fn set_batch_running(running: bool) {
    RUNNING.store(running, Ordering::Relaxed);
}

#[tauri::command]
pub fn set_auto_mode(on: bool) {
    AUTO.store(on, Ordering::Relaxed);
}

#[tauri::command]
pub fn is_media_processed(fingerprint: String) -> Result<bool, String> {
    db::with(|conn| db::sources::is_auto_processed(conn, &fingerprint))
}

#[tauri::command]
pub fn mark_media_processed(fingerprint: String) -> Result<(), String> {
    db::with(|conn| db::sources::set_auto_processed(conn, &fingerprint, Some(db::now_ms())))
}

#[tauri::command]
pub fn get_last_batch() -> Result<Option<BatchRecord>, String> {
    db::with(|conn| {
        db::batches::latest_id(conn)?.map(|id| db::batches::read(conn, &id)).transpose()
    })?
    .flatten()
    .map(BatchRecord::from_stored)
    .transpose()
}

#[tauri::command]
pub async fn create_batch(
    state: State<'_, Mutex<AppState>>,
    source: BatchSource,
    mut items: Vec<BatchItem>,
    auto: bool,
) -> Result<BatchRecord, String> {
    let _operation = OPERATION.try_lock().map_err(|_| "Mining or undo is already running")?;
    if items.is_empty() {
        return Err("The batch is empty".into());
    }
    let current = BatchSource::from_file(&state.lock().unwrap().file)?;
    if !source.matches(&current) {
        return Err("The source changed. Review the queue before mining".into());
    }
    for item in &mut items {
        item.outcome = Outcome::Unattempted;
    }
    let now = chrono::Utc::now();
    let batch = BatchRecord {
        id: now.timestamp_nanos_opt().ok_or("Could not assign a batch ID")?.to_string(),
        started_at: now.timestamp_millis(),
        finished_at: None,
        source: current,
        items,
        auto,
    };
    db::with(|conn| db::batches::write(conn, &batch.stored()))?;
    Ok(batch)
}

#[tauri::command]
pub async fn finish_batch(batch_id: String) -> Result<BatchRecord, String> {
    let _operation = OPERATION.try_lock().map_err(|_| "Mining or undo is already running")?;
    let mut batch = load(&batch_id)?;
    batch.finished_at = Some(chrono::Utc::now().timestamp_millis());
    save(&batch).map_err(|e| e.message)?;
    Ok(batch)
}

#[derive(Serialize)]
pub struct UndoResult {
    pub batch: BatchRecord,
    pub deleted: usize,
    pub already_gone: usize,
    pub remaining: usize,
    /// Auto mode can mine the source again.
    pub reopened: bool,
}

#[tauri::command]
pub async fn undo_batch(batch_id: String) -> Result<UndoResult, String> {
    let _operation = OPERATION.try_lock().map_err(|_| "Mining or undo is already running")?;
    let mut batch = load(&batch_id)?;
    let ids: Vec<_> = batch
        .items
        .iter()
        .filter_map(|i| match i.outcome {
            Outcome::Created { note_id, .. } => Some(note_id),
            _ => None,
        })
        .collect();
    let existing = api::existing_note_ids_strict(&ids).await?;
    if !existing.is_empty() {
        api::delete_notes(&existing).await?;
    }
    let remaining = api::existing_note_ids_strict(&existing).await?;
    mark_deleted(&mut batch, &remaining);
    save(&batch).map_err(|e| format!("Anki deletion was checked, but recovery history could not be saved: {}. Retry Undo to reconcile it.", e.message))?;
    let gone: Vec<u64> = ids.iter().copied().filter(|id| !remaining.contains(id)).collect();
    mined::mark_notes_deleted(&gone)?;
    let reopened = batch.auto && remaining.is_empty();
    if reopened {
        db::with(|conn| db::sources::set_auto_processed(conn, &batch.source.fingerprint, None))?;
    }
    Ok(UndoResult {
        deleted: existing.len() - remaining.len(),
        already_gone: ids.len() - existing.len(),
        remaining: remaining.len(),
        reopened,
        batch,
    })
}

fn mark_deleted(batch: &mut BatchRecord, remaining: &[u64]) {
    for item in &mut batch.items {
        if let Outcome::Created { note_id, .. } = item.outcome {
            if !remaining.contains(&note_id) {
                item.outcome = Outcome::Deleted { note_id };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch() -> BatchRecord {
        BatchRecord {
            id: "first".into(),
            started_at: 1,
            finished_at: None,
            source: BatchSource {
                title: "Test".into(),
                locator: "test.srt".into(),
                fingerprint: "test".into(),
            },
            items: vec![BatchItem {
                key: "猫 ねこ".into(),
                lemma: "猫".into(),
                surface: "猫".into(),
                sentence: "猫だ".into(),
                timestamp: None,
                entry_index: None,
                format_name: None,
                scan_text: None,
                adhoc: false,
                mine_media: true,
                outcome: Outcome::Created { note_id: 42, media: MediaState::Pending, error: None },
            }],
            auto: false,
        }
    }

    #[test]
    fn stored_items_read_back_as_the_same_outcomes() {
        let original = batch();
        let restored = BatchRecord::from_stored(original.stored()).unwrap();
        assert!(matches!(
            restored.items[0].outcome,
            Outcome::Created { note_id: 42, media: MediaState::Pending, .. }
        ));
        assert!(restored.source == original.source);
    }

    #[test]
    fn undo_only_marks_confirmed_absent_created_notes() {
        let mut record = batch();
        for outcome in [
            Outcome::Duplicate,
            Outcome::Attempting,
            Outcome::Unattempted,
            Outcome::Created { note_id: 43, media: MediaState::Complete, error: None },
        ] {
            let mut item = record.items[0].clone();
            item.outcome = outcome;
            record.items.push(item);
        }
        mark_deleted(&mut record, &[43]);
        assert!(matches!(record.items[0].outcome, Outcome::Deleted { note_id: 42 }));
        assert!(matches!(record.items[1].outcome, Outcome::Duplicate));
        assert!(matches!(record.items[2].outcome, Outcome::Attempting));
        assert!(matches!(record.items[3].outcome, Outcome::Unattempted));
        assert!(matches!(record.items[4].outcome, Outcome::Created { note_id: 43, .. }));
    }
}
