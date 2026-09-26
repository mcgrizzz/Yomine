//! Batches and their items. Items are stored as the Tauri crate's `BatchItem` JSON.

use rusqlite::{
    params,
    Connection,
    OptionalExtension,
};

use super::sources;

pub struct StoredBatch {
    pub id: String,
    pub fingerprint: String,
    pub title: String,
    pub path: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub auto: bool,
    /// The Anki profile it mines into; empty for batches from before profiles were
    /// recorded, which count for every profile.
    pub collection: Option<String>,
    pub items: Vec<String>,
}

/// The newest batch of an Anki profile, or of any profile for `None`.
pub fn latest_id(conn: &Connection, collection: Option<&str>) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT id FROM batches WHERE ?1 IS NULL OR collection = ?1 OR collection IS NULL
         ORDER BY started_at DESC, id DESC LIMIT 1",
        params![collection],
        |r| r.get(0),
    )
    .optional()
}

pub fn read(conn: &Connection, id: &str) -> rusqlite::Result<Option<StoredBatch>> {
    let Some(mut batch) = conn
        .query_row(
            "SELECT b.started_at, b.finished_at, b.auto, b.path, s.fingerprint, coalesce(s.title, ''),
                 b.collection
             FROM batches b JOIN sources s ON s.id = b.source_id WHERE b.id = ?1",
            params![id],
            |r| {
                Ok(StoredBatch {
                    id: id.to_string(),
                    started_at: r.get(0)?,
                    finished_at: r.get(1)?,
                    auto: r.get(2)?,
                    path: r.get(3)?,
                    fingerprint: r.get(4)?,
                    title: r.get(5)?,
                    collection: r.get(6)?,
                    items: Vec::new(),
                })
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    batch.items = conn
        .prepare("SELECT item FROM batch_items WHERE batch_id = ?1 ORDER BY position")?
        .query_map(params![id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(batch))
}

/// Inserts or replaces the batch and its items; the caller owns the transaction.
pub(super) fn put(conn: &Connection, batch: &StoredBatch) -> rusqlite::Result<i64> {
    let source = sources::id(conn, &batch.fingerprint)?;
    conn.execute(
        "INSERT INTO batches (id, source_id, path, started_at, finished_at, auto, collection)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (id) DO UPDATE SET finished_at = excluded.finished_at",
        params![
            batch.id,
            source,
            batch.path,
            batch.started_at,
            batch.finished_at,
            batch.auto,
            batch.collection
        ],
    )?;
    conn.execute("DELETE FROM batch_items WHERE batch_id = ?1", params![batch.id])?;
    for (position, item) in batch.items.iter().enumerate() {
        conn.execute(
            "INSERT INTO batch_items (batch_id, position, item) VALUES (?1, ?2, ?3)",
            params![batch.id, position as i64, item],
        )?;
    }
    Ok(source)
}

pub fn write(conn: &mut Connection, batch: &StoredBatch) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let source = put(&tx, batch)?;
    sources::set_title(&tx, source, &batch.title)?;
    tx.commit()
}

/// Only a profile's newest batch is resumable, so an older one is never saved over.
pub fn save_latest(conn: &mut Connection, batch: &StoredBatch) -> rusqlite::Result<bool> {
    if latest_id(conn, batch.collection.as_deref())?.as_deref() != Some(batch.id.as_str()) {
        return Ok(false);
    }
    write(conn, batch).map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(id: &str, started_at: i64, collection: Option<&str>) -> StoredBatch {
        StoredBatch {
            id: id.into(),
            fingerprint: "test".into(),
            title: "Test".into(),
            path: "test.srt".into(),
            started_at,
            finished_at: None,
            auto: false,
            collection: collection.map(Into::into),
            items: vec![r#"{"note":42}"#.into()],
        }
    }

    #[test]
    fn each_profile_resumes_only_its_newest_batch() {
        let (dir, mut conn) = super::super::tests::scratch("batches");
        write(&mut conn, &batch("legacy", 1, None)).unwrap();
        write(&mut conn, &batch("main", 2, Some("Main"))).unwrap();
        drop(conn);

        let mut conn = super::super::open(&dir.join(super::super::DB_FILE), &dir).unwrap();
        assert_eq!(read(&conn, "main").unwrap().unwrap().items, [r#"{"note":42}"#]);
        // A batch from before profiles were recorded counts for every profile.
        assert_eq!(latest_id(&conn, Some("Other")).unwrap().as_deref(), Some("legacy"));
        write(&mut conn, &batch("other", 3, Some("Other"))).unwrap();

        assert_eq!(latest_id(&conn, Some("Main")).unwrap().as_deref(), Some("main"));
        assert!(save_latest(&mut conn, &batch("main", 2, Some("Main"))).unwrap());
        assert!(!save_latest(&mut conn, &batch("legacy", 1, None)).unwrap());
        assert_eq!(latest_id(&conn, None).unwrap().as_deref(), Some("other"));
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
