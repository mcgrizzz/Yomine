//! Sources, their loads (the recent-files list) and the EPUB parts seen.

use rusqlite::{
    params,
    Connection,
};

use crate::core::recent_files::RecentFileEntry;

/// The source's row id, adding a bare row for a fingerprint not seen before.
pub fn id(conn: &Connection, fingerprint: &str) -> rusqlite::Result<i64> {
    conn.execute("INSERT OR IGNORE INTO sources (fingerprint) VALUES (?1)", params![fingerprint])?;
    conn.query_row("SELECT id FROM sources WHERE fingerprint = ?1", params![fingerprint], |r| {
        r.get(0)
    })
}

pub struct SourceInfo<'a> {
    pub fingerprint: &'a str,
    pub kind: &'a str,
    pub title: &'a str,
    pub creator: Option<&'a str>,
    pub char_count: usize,
    pub runtime_ms: Option<i64>,
}

fn upsert(conn: &Connection, info: &SourceInfo) -> rusqlite::Result<i64> {
    let id = id(conn, info.fingerprint)?;
    conn.execute(
        "UPDATE sources SET kind = ?2, title = ?3, creator = ?4, char_count = ?5, runtime_ms = ?6
         WHERE id = ?1",
        params![id, info.kind, info.title, info.creator, info.char_count as i64, info.runtime_ms],
    )?;
    Ok(id)
}

pub fn set_title(conn: &Connection, id: i64, title: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE sources SET title = ?2 WHERE id = ?1", params![id, title]).map(|_| ())
}

/// Whether auto mode mined the source into this Anki profile.
pub fn is_auto_processed(
    conn: &Connection,
    fingerprint: &str,
    collection: &str,
) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM auto_processed a JOIN sources s ON s.id = a.source_id
                        WHERE s.fingerprint = ?1 AND a.collection IN (?2, ''))",
        params![fingerprint, collection],
        |r| r.get(0),
    )
}

/// `None` lets auto mode process the source again in that profile.
pub fn set_auto_processed(
    conn: &Connection,
    fingerprint: &str,
    collection: &str,
    at: Option<i64>,
) -> rusqlite::Result<()> {
    let id = id(conn, fingerprint)?;
    match at {
        Some(at) => conn.execute(
            "INSERT OR REPLACE INTO auto_processed (source_id, collection, at) VALUES (?1, ?2, ?3)",
            params![id, collection, at],
        ),
        None => conn.execute(
            "DELETE FROM auto_processed WHERE source_id = ?1 AND collection = ?2",
            params![id, collection],
        ),
    }
    .map(|_| ())
}

/// One load of a file, as the recent-files list shows it.
pub struct Open<'a> {
    pub path: &'a str,
    pub title: &'a str,
    pub label: Option<&'a str>,
    pub creator: Option<&'a str>,
    pub term_count: Option<i64>,
    pub file_size: Option<i64>,
    pub opened_at: i64,
}

pub(super) fn insert_open(
    conn: &Connection,
    source: Option<i64>,
    open: &Open,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO opens (source_id, path, title, label, creator, term_count, file_size, opened_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            source,
            open.path,
            open.title,
            open.label,
            open.creator,
            open.term_count,
            open.file_size,
            open.opened_at
        ],
    )
    .map(|_| ())
}

pub fn record_open(
    conn: &Connection,
    info: &SourceInfo,
    open: &Open,
    epub_parts: &[usize],
) -> rusqlite::Result<()> {
    let source = upsert(conn, info)?;
    insert_open(conn, Some(source), open)?;
    mark_parts_seen(conn, open.path, epub_parts.iter().map(|p| *p as i64))
}

/// Each path's latest load, most recent first.
pub fn recent(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<RecentFileEntry>> {
    // SQLite takes the other columns from the row holding max(opened_at).
    conn.prepare(
        "SELECT path, title, label, creator, max(opened_at), file_size, term_count
         FROM opens GROUP BY path ORDER BY max(opened_at) DESC LIMIT ?1",
    )?
    .query_map(params![limit as i64], |r| {
        Ok(RecentFileEntry {
            file_path: r.get(0)?,
            title: r.get(1)?,
            subtitle: r.get(2)?,
            creator: r.get(3)?,
            last_opened: chrono::DateTime::from_timestamp_millis(r.get(4)?).unwrap_or_default(),
            file_size: r.get::<_, Option<i64>>(5)?.map(|n| n as u64),
            term_count: r.get::<_, Option<i64>>(6)?.map(|n| n as usize),
        })
    })?
    .collect()
}

pub(super) fn mark_parts_seen(
    conn: &Connection,
    path: &str,
    parts: impl IntoIterator<Item = i64>,
) -> rusqlite::Result<()> {
    for part in parts {
        conn.execute(
            "INSERT OR IGNORE INTO epub_parts_seen (path, part_id) VALUES (?1, ?2)",
            params![path, part],
        )?;
    }
    Ok(())
}

pub fn epub_parts_seen(conn: &Connection, path: &str) -> rusqlite::Result<Vec<usize>> {
    conn.prepare("SELECT part_id FROM epub_parts_seen WHERE path = ?1")?
        .query_map(params![path], |r| r.get::<_, i64>(0).map(|id| id as usize))?
        .collect()
}
