//! Notes Yomine created in Anki.

use rusqlite::{
    params,
    Connection,
};

use crate::anki::mined::MinedSentence;

pub struct NewNote<'a> {
    pub note_id: u64,
    /// `normalize_sentence`'s form.
    pub sentence: &'a str,
    pub term: Option<&'a str>,
    pub source: Option<i64>,
    pub batch_id: Option<&'a str>,
    pub deleted_at: Option<i64>,
}

/// Keeps the first record of a note id.
pub fn insert(conn: &Connection, note: &NewNote) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO notes (note_id, sentence, term, source_id, batch_id, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            note.note_id as i64,
            note.sentence,
            note.term,
            note.source,
            note.batch_id,
            note.deleted_at
        ],
    )
    .map(|_| ())
}

pub fn mark_deleted(conn: &Connection, note_ids: &[u64], at: i64) -> rusqlite::Result<()> {
    for id in note_ids {
        conn.execute(
            "UPDATE notes SET deleted_at = ?2 WHERE note_id = ?1 AND deleted_at IS NULL",
            params![*id as i64, at],
        )?;
    }
    Ok(())
}

/// Notes still in Anki as far as Yomine knows, with a sentence to match on.
pub fn live_sentences(conn: &Connection) -> rusqlite::Result<Vec<MinedSentence>> {
    conn.prepare("SELECT note_id, sentence FROM notes WHERE deleted_at IS NULL AND sentence != ''")?
        .query_map([], |r| {
            Ok(MinedSentence { note_id: r.get::<_, i64>(0)? as u64, sentence: r.get(1)? })
        })?
        .collect()
}
