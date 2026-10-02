//! Sources, their loads (the recent-files list) and the EPUB parts seen.

use rusqlite::{
    params,
    Connection,
    OptionalExtension,
};

use crate::core::{
    recent_files::RecentFileEntry,
    settings::MiningMode,
};

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

/// The video paired with the source, if it still exists.
pub fn video(conn: &Connection, fingerprint: &str) -> rusqlite::Result<Option<String>> {
    let path: Option<String> = conn
        .query_row("SELECT video FROM sources WHERE fingerprint = ?1", params![fingerprint], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    Ok(path.filter(|p| std::path::Path::new(p).is_file()))
}

pub fn set_video(conn: &Connection, fingerprint: &str, path: Option<&str>) -> rusqlite::Result<()> {
    let id = id(conn, fingerprint)?;
    conn.execute("UPDATE sources SET video = ?2 WHERE id = ?1", params![id, path]).map(|_| ())
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
    pub mode: Option<MiningMode>,
}

fn mode_key(mode: MiningMode) -> &'static str {
    match mode {
        MiningMode::Local => "local",
        MiningMode::Asbplayer => "asbplayer",
    }
}

pub(super) fn insert_open(
    conn: &Connection,
    source: Option<i64>,
    open: &Open,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO opens
             (source_id, path, title, label, creator, term_count, file_size, opened_at, mode)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            source,
            open.path,
            open.title,
            open.label,
            open.creator,
            open.term_count,
            open.file_size,
            open.opened_at,
            open.mode.map(mode_key)
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

/// Each source's latest load in each mining mode, most recent first. A video and the
/// subtitles it loads are one source, so opening the episode either way leaves one entry
/// per mode; a book has one entry and no mode, since it mines the same in both.
pub fn recent(conn: &Connection, limit: usize) -> rusqlite::Result<Vec<RecentFileEntry>> {
    // Loads from before modes were recorded: asbplayer's subtitles are saved under
    // asbplayer_subtitles, and anything else was opened from disk. Loads imported
    // without a source are told apart as books by their extension.
    // SQLite takes the other columns from the row holding max(opened_at).
    conn.prepare(
        "WITH loads AS (
             SELECT o.path, o.title, o.label, o.creator, o.opened_at, o.file_size,
                    o.term_count, o.source_id,
                    CASE
                        WHEN s.kind IN ('epub', 'text')
                          OR (o.source_id IS NULL
                              AND (lower(o.path) LIKE '%.epub' OR lower(o.path) LIKE '%.txt'))
                        THEN NULL
                        ELSE coalesce(o.mode, CASE WHEN instr(o.path, 'asbplayer_subtitles') > 0
                                                   THEN 'asbplayer' ELSE 'local' END)
                    END AS mode
             FROM opens o LEFT JOIN sources s ON s.id = o.source_id),
         latest AS (
             SELECT path, title, label, creator, max(opened_at) AS opened_at, file_size,
                    term_count, source_id, mode
             FROM loads GROUP BY path, mode)
         SELECT path, title, label, creator, opened_at, file_size, term_count, mode
         FROM latest AS l
         WHERE source_id IS NULL
            OR opened_at = (SELECT max(opened_at) FROM latest
                            WHERE source_id = l.source_id AND mode IS l.mode)
         ORDER BY opened_at DESC LIMIT ?1",
    )?
    .query_map(params![limit as i64], |r| {
        let mode = match r.get::<_, Option<String>>(7)?.as_deref() {
            Some("local") => Some(MiningMode::Local),
            Some(_) => Some(MiningMode::Asbplayer),
            None => None,
        };
        Ok(RecentFileEntry {
            file_path: r.get(0)?,
            title: r.get(1)?,
            subtitle: r.get(2)?,
            creator: r.get(3)?,
            last_opened: chrono::DateTime::from_timestamp_millis(r.get(4)?).unwrap_or_default(),
            file_size: r.get::<_, Option<i64>>(5)?.map(|n| n as u64),
            term_count: r.get::<_, Option<i64>>(6)?.map(|n| n as usize),
            mode,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::db::tests::scratch;

    fn load(
        conn: &Connection,
        fingerprint: &str,
        kind: &str,
        path: &str,
        at: i64,
        mode: MiningMode,
    ) {
        let info = SourceInfo {
            fingerprint,
            kind,
            title: path,
            creator: None,
            char_count: 0,
            runtime_ms: None,
        };
        let open = Open {
            path,
            title: path,
            label: None,
            creator: None,
            term_count: None,
            file_size: None,
            opened_at: at,
            mode: Some(mode),
        };
        record_open(conn, &info, &open, &[]).unwrap();
    }

    #[test]
    fn each_mode_lists_its_own_latest_load_and_books_show_once() {
        let (dir, conn) = scratch("recent");
        let asb = "C:/data/asbplayer_subtitles/ep1.srt";
        load(&conn, "ep1", "subtitles", "D:/anime/ep1.mkv", 1, MiningMode::Local);
        load(&conn, "ep1", "subtitles", asb, 2, MiningMode::Asbplayer);
        load(&conn, "ep1", "subtitles", "D:/anime/ep1.srt", 3, MiningMode::Local);
        load(&conn, "book", "epub", "D:/books/a.epub", 4, MiningMode::Local);
        load(&conn, "book", "epub", "D:/books/a.epub", 5, MiningMode::Asbplayer);
        // A load from before modes were recorded.
        let old = Open {
            path: "C:/data/asbplayer_subtitles/ep2.srt",
            title: "ep2",
            label: None,
            creator: None,
            term_count: None,
            file_size: None,
            opened_at: 0,
            mode: None,
        };
        insert_open(&conn, None, &old).unwrap();

        let listed: Vec<(String, Option<MiningMode>)> =
            recent(&conn, 50).unwrap().into_iter().map(|e| (e.file_path, e.mode)).collect();
        assert_eq!(
            listed,
            [
                ("D:/books/a.epub".into(), None),
                ("D:/anime/ep1.srt".into(), Some(MiningMode::Local)),
                (asb.into(), Some(MiningMode::Asbplayer)),
                ("C:/data/asbplayer_subtitles/ep2.srt".into(), Some(MiningMode::Asbplayer)),
            ]
        );
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
