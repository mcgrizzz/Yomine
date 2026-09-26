//! Mining history and Anki mirrors in one SQLite file per profile. Settings stay JSON:
//! they're small and hand-editable.

use std::{
    path::Path,
    sync::Mutex,
};

use rusqlite::{
    params,
    Connection,
};

pub mod anki;
pub mod batches;
mod import;
pub mod notes;
pub mod sources;

pub const DB_FILE: &str = "yomine.db";

const SCHEMA: &str = "
-- A show, movie or book. Empty until sources are matched to TVDB/AniDB/AniList.
CREATE TABLE works (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    tvdb_id INTEGER,
    anidb_id INTEGER,
    anilist_id INTEGER
);

CREATE TABLE episodes (
    id INTEGER PRIMARY KEY,
    work_id INTEGER NOT NULL REFERENCES works(id),
    season INTEGER,
    number INTEGER,
    title TEXT,
    tvdb_id INTEGER,
    anidb_id INTEGER
);

-- One row per distinct content, keyed by the fingerprint batches compute.
CREATE TABLE sources (
    id INTEGER PRIMARY KEY,
    fingerprint TEXT NOT NULL UNIQUE,
    kind TEXT,
    title TEXT,
    creator TEXT,
    char_count INTEGER,
    -- The last subtitle line's end, close to the episode's length.
    runtime_ms INTEGER,
    auto_processed_at INTEGER,
    work_id INTEGER REFERENCES works(id),
    episode_id INTEGER REFERENCES episodes(id)
);

-- Each load of a file, as the recent-files list shows it. Loads imported from
-- recent_files.json have no source: that file never recorded a fingerprint.
CREATE TABLE opens (
    id INTEGER PRIMARY KEY,
    source_id INTEGER REFERENCES sources(id),
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    label TEXT,
    creator TEXT,
    term_count INTEGER,
    file_size INTEGER,
    opened_at INTEGER NOT NULL
);
CREATE INDEX opens_by_path ON opens(path, opened_at);

CREATE TABLE epub_parts_seen (
    path TEXT NOT NULL,
    part_id INTEGER NOT NULL,
    PRIMARY KEY (path, part_id)
);

CREATE TABLE batches (
    id TEXT PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES sources(id),
    path TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    auto INTEGER NOT NULL
);

CREATE TABLE batch_items (
    batch_id TEXT NOT NULL REFERENCES batches(id),
    position INTEGER NOT NULL,
    -- The Tauri crate's BatchItem, serialized.
    item TEXT NOT NULL,
    PRIMARY KEY (batch_id, position)
);

-- Notes Yomine created. An Anki note id is its creation time in epoch milliseconds.
CREATE TABLE notes (
    note_id INTEGER PRIMARY KEY,
    -- normalize_sentence's form, the already-mined match key.
    sentence TEXT NOT NULL,
    term TEXT,
    source_id INTEGER REFERENCES sources(id),
    batch_id TEXT REFERENCES batches(id),
    -- Set by Undo, or when Anki no longer has the note.
    deleted_at INTEGER
);
";

/// Anki's cards and sentence fields per collection (the Anki profile name), with the
/// dates Yomine first and last saw them. Rows imported from the JSON caches start in
/// the unknown collection '' until the first harvest claims them.
const SCHEMA_V2: &str = "
CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Every note a harvest has seen, vocab or not, so the next one fetches only new and
-- edited notes.
CREATE TABLE anki_notes (
    collection TEXT NOT NULL,
    note_id INTEGER NOT NULL,
    first_seen_at INTEGER NOT NULL,
    gone_at INTEGER,
    PRIMARY KEY (collection, note_id)
);

CREATE TABLE anki_cards (
    collection TEXT NOT NULL,
    card_id INTEGER NOT NULL,
    -- Empty for cards imported from the JSON cache, until a harvest reads their note.
    note_id INTEGER,
    term TEXT NOT NULL,
    reading TEXT NOT NULL,
    -- Days; fractional while learning.
    interval REAL,
    first_seen_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    gone_at INTEGER,
    PRIMARY KEY (collection, card_id)
);

-- A card's scheduling state each time a harvest saw it change, from its first sighting.
CREATE TABLE anki_card_history (
    collection TEXT NOT NULL,
    card_id INTEGER NOT NULL,
    at INTEGER NOT NULL,
    interval REAL
);
CREATE INDEX anki_card_history_by_card ON anki_card_history(collection, card_id, at);

CREATE TABLE anki_sentences (
    collection TEXT NOT NULL,
    note_id INTEGER NOT NULL,
    -- normalize_sentence's form.
    sentence TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    gone_at INTEGER,
    PRIMARY KEY (collection, note_id)
);

-- Empty for notes mined before collections were recorded.
ALTER TABLE notes ADD COLUMN collection TEXT;
";

/// Batches and auto mode's processed videos per Anki profile. Rows from before this
/// have no profile and count for every one.
const SCHEMA_V3: &str = "
ALTER TABLE batches ADD COLUMN collection TEXT;

CREATE TABLE auto_processed (
    source_id INTEGER NOT NULL REFERENCES sources(id),
    -- The Anki profile the video was mined into; '' counts for every profile.
    collection TEXT NOT NULL,
    at INTEGER NOT NULL,
    PRIMARY KEY (source_id, collection)
);
INSERT INTO auto_processed (source_id, collection, at)
    SELECT id, '', auto_processed_at FROM sources WHERE auto_processed_at IS NOT NULL;
ALTER TABLE sources DROP COLUMN auto_processed_at;

-- Notes have recorded their profile since v2, which places the batches they came from.
UPDATE batches SET collection = (
    SELECT n.collection FROM notes n WHERE n.batch_id = batches.id AND n.collection IS NOT NULL
);
UPDATE auto_processed SET collection = coalesce((
    SELECT b.collection FROM batches b
    WHERE b.source_id = auto_processed.source_id AND b.auto AND b.collection IS NOT NULL
    ORDER BY b.started_at DESC LIMIT 1
), '');
";

static DB: Mutex<Option<Connection>> = Mutex::new(None);

/// Runs `f` on the active profile's database, opening it on first use.
pub fn with<T>(f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T, String> {
    let mut guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let dir = super::get_profile_dir();
        *guard = Some(
            open(&dir.join(DB_FILE), &dir)
                .map_err(|e| format!("Could not open the history database: {e}"))?,
        );
    }
    f(guard.as_mut().expect("opened above")).map_err(|e| format!("History database error: {e}"))
}

/// Opens or creates the database, importing the profile's JSON history the first time.
pub fn open(path: &Path, json_dir: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let tx = conn.transaction()?;
    // Every schema step runs before any import, which writes the latest columns.
    if version < 1 {
        tx.execute_batch(SCHEMA)?;
    }
    if version < 2 {
        tx.execute_batch(SCHEMA_V2)?;
    }
    if version < 3 {
        tx.execute_batch(SCHEMA_V3)?;
    }
    if version < 1 {
        import::import_json(&tx, json_dir)?;
    }
    if version < 2 {
        import::import_anki_caches(&tx, json_dir)?;
    }
    tx.pragma_update(None, "user_version", 3)?;
    tx.commit()?;
    Ok(conn)
}

/// A consistent copy of a database another connection may have open.
pub fn copy(from: &Path, to: &Path) -> rusqlite::Result<()> {
    let conn = Connection::open_with_flags(from, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.execute("VACUUM INTO ?1", params![to.to_string_lossy()])?;
    Ok(())
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// A fresh database in its own temp directory.
    pub(crate) fn scratch(label: &str) -> (std::path::PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!(
            "yomine-db-{label}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = open(&dir.join(DB_FILE), &dir).unwrap();
        (dir, conn)
    }

    #[test]
    fn a_copy_includes_writes_still_in_the_open_database_log() {
        let (dir, conn) = scratch("copy");
        conn.execute("INSERT INTO notes (note_id, sentence) VALUES (1, '猫だ')", []).unwrap();

        copy(&dir.join(DB_FILE), &dir.join("copy.db")).unwrap();
        let copied = open(&dir.join("copy.db"), &dir).unwrap();
        assert_eq!(count(&copied, "SELECT count(*) FROM notes"), 1);
        drop((conn, copied));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
