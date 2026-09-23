use std::{
    path::PathBuf,
    sync::Mutex,
};

use serde::{
    Deserialize,
    Serialize,
};
use tauri::State;
use yomine::{
    anki::api,
    persistence,
};

use crate::{
    dto::TimeStampDto,
    state::{
        AppState,
        FileData,
    },
};

const FILE: &str = "yomine_last_batch.json";
static FILE_LOCK: Mutex<()> = Mutex::new(());
pub static OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct BatchSource {
    pub title: String,
    pub locator: String,
    pub fingerprint: String,
}

impl BatchSource {
    pub fn from_file(file: &FileData) -> Result<Self, String> {
        let source = file.source_file.as_ref().ok_or("Load the original source first")?;
        let sentences: Vec<_> = file
            .sentences
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
        Ok(Self {
            title: source.title.clone(),
            locator: source.original_file.clone(),
            fingerprint: format!("{hash:016x}"),
        })
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
}

struct BatchFile(PathBuf);

#[derive(Debug)]
enum LoadError {
    Io(String),
    Unreadable(String),
}

impl From<LoadError> for String {
    fn from(error: LoadError) -> Self {
        match error {
            LoadError::Io(e) => format!("Could not read the last batch: {e}"),
            LoadError::Unreadable(e) => format!("The last batch record is unreadable: {e}"),
        }
    }
}

enum Stored {
    Loaded(Option<BatchRecord>),
    MovedAside(PathBuf),
}

impl BatchFile {
    fn load(&self) -> Result<Option<BatchRecord>, LoadError> {
        let json = match std::fs::read_to_string(&self.0) {
            Ok(json) => json,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LoadError::Io(e.to_string())),
        };
        serde_json::from_str(&json).map(Some).map_err(|e| LoadError::Unreadable(e.to_string()))
    }
    fn load_or_move_aside(&self) -> Result<Stored, String> {
        match self.load() {
            Err(LoadError::Unreadable(_)) => {
                let moved = self.0.with_file_name(format!(
                    "yomine_last_batch.unreadable-{}.json",
                    chrono::Utc::now().timestamp_millis()
                ));
                std::fs::rename(&self.0, &moved)
                    .map_err(|e| format!("Could not move the unreadable batch record: {e}"))?;
                Ok(Stored::MovedAside(moved))
            }
            loaded => Ok(Stored::Loaded(loaded?)),
        }
    }
    fn write(&self, batch: &BatchRecord) -> Result<(), String> {
        persistence::save_json_at(batch, &self.0)
            .map_err(|e| format!("Could not save the last batch: {e}"))
    }
    fn save(&self, batch: &BatchRecord) -> Result<(), String> {
        let current = self.load()?.ok_or("No saved batch")?;
        if current.id != batch.id {
            return Err("This batch has been replaced by a newer batch".into());
        }
        self.write(batch)
    }
}

fn file() -> BatchFile {
    BatchFile(persistence::get_data_file_path(FILE))
}

pub fn load(id: &str) -> Result<BatchRecord, String> {
    get_last_batch()?
        .filter(|b| b.id == id)
        .ok_or_else(|| "This batch is no longer available".into())
}

pub fn save(batch: &BatchRecord) -> Result<(), Failure> {
    let _guard = FILE_LOCK.lock().unwrap();
    file().save(batch).map_err(Failure::storage)
}

pub fn checkpoint(batch: &mut BatchRecord, index: usize, outcome: Outcome) -> Result<(), Failure> {
    batch.items.get_mut(index).ok_or_else(|| Failure::storage("Batch item not found"))?.outcome =
        outcome;
    batch.finished_at = None;
    save(batch)
}

#[tauri::command]
pub fn get_last_batch() -> Result<Option<BatchRecord>, String> {
    let _guard = FILE_LOCK.lock().unwrap();
    match file().load_or_move_aside()? {
        Stored::Loaded(batch) => Ok(batch),
        Stored::MovedAside(moved) => Err(format!(
            "The last batch record was unreadable, so it was moved to {}. New batches will start \
             a fresh record.",
            moved.display()
        )),
    }
}

#[tauri::command]
pub async fn create_batch(
    state: State<'_, Mutex<AppState>>,
    source: BatchSource,
    mut items: Vec<BatchItem>,
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
    };
    let _guard = FILE_LOCK.lock().unwrap();
    file().load_or_move_aside()?;
    file().write(&batch)?;
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
    Ok(UndoResult {
        deleted: existing.len() - remaining.len(),
        already_gone: ids.len() - existing.len(),
        remaining: remaining.len(),
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
        }
    }

    #[test]
    fn saved_creation_survives_restart_and_stale_updates_cannot_replace_it() {
        let dir = std::env::temp_dir().join(format!(
            "yomine-batch-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let store = BatchFile(dir.join(FILE));
        assert!(store.load().unwrap().is_none());
        let original = batch();
        store.write(&original).unwrap();
        assert!(matches!(
            store.load().unwrap().unwrap().items[0].outcome,
            Outcome::Created { note_id: 42, media: MediaState::Pending, .. }
        ));
        let mut newer = original.clone();
        newer.id = "second".into();
        store.write(&newer).unwrap();
        assert!(store.save(&original).is_err());
        assert_eq!(store.load().unwrap().unwrap().id, "second");
        std::fs::write(&store.0, b"broken json").unwrap();
        assert!(store.load().is_err());
        assert!(store.save(&newer).is_err());
        assert_eq!(std::fs::read(&store.0).unwrap(), b"broken json");
        let Stored::MovedAside(moved) = store.load_or_move_aside().unwrap() else {
            panic!("not moved")
        };
        assert_eq!(std::fs::read(&moved).unwrap(), b"broken json");
        assert!(matches!(store.load_or_move_aside().unwrap(), Stored::Loaded(None)));
        std::fs::remove_dir_all(dir).unwrap();
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
