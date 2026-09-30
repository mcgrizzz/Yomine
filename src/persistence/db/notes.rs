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
    /// The Anki profile the note went into.
    pub collection: Option<&'a str>,
    pub deleted_at: Option<i64>,
}

/// Keeps the first record of a note id.
pub fn insert(conn: &Connection, note: &NewNote) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO notes
             (note_id, sentence, term, source_id, batch_id, collection, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            note.note_id as i64,
            note.sentence,
            note.term,
            note.source,
            note.batch_id,
            note.collection,
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

/// The collection's notes still in Anki as far as Yomine knows, with a sentence to
/// match on. Notes from before collections were recorded count for every collection.
pub fn live_sentences(conn: &Connection, collection: &str) -> rusqlite::Result<Vec<MinedSentence>> {
    conn.prepare(
        "SELECT note_id, sentence FROM notes
         WHERE deleted_at IS NULL AND sentence != '' AND (collection = ?1 OR collection IS NULL)",
    )?
    .query_map(params![collection], |r| {
        Ok(MinedSentence { note_id: r.get::<_, i64>(0)? as u64, sentence: r.get(1)? })
    })?
    .collect()
}

/// Those of `note_ids` Yomine has recorded, deleted or not.
pub fn recorded(conn: &Connection, note_ids: &[u64]) -> rusqlite::Result<Vec<u64>> {
    let mut statement = conn.prepare("SELECT 1 FROM notes WHERE note_id = ?1")?;
    let mut found = Vec::new();
    for id in note_ids {
        if statement.exists(params![*id as i64])? {
            found.push(*id);
        }
    }
    Ok(found)
}

/// The collection's notes Yomine hasn't seen deleted.
pub fn live_ids(conn: &Connection, collection: &str) -> rusqlite::Result<Vec<u64>> {
    conn.prepare("SELECT note_id FROM notes WHERE deleted_at IS NULL AND collection = ?1")?
        .query_map(params![collection], |r| r.get::<_, i64>(0).map(|id| id as u64))?
        .collect()
}

/// Terms Yomine mined from a source into the collection, whose notes it hasn't seen
/// deleted.
pub fn live_terms(
    conn: &Connection,
    collection: &str,
    fingerprint: &str,
) -> rusqlite::Result<Vec<String>> {
    conn.prepare(
        "SELECT DISTINCT n.term FROM notes n JOIN sources s ON s.id = n.source_id
         WHERE n.deleted_at IS NULL AND n.term IS NOT NULL AND s.fingerprint = ?2
           AND (n.collection = ?1 OR n.collection IS NULL)",
    )?
    .query_map(params![collection, fingerprint], |r| r.get(0))?
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        super::{
            sources,
            tests::scratch,
        },
        *,
    };

    #[test]
    fn mined_terms_are_the_sources_live_notes_in_the_collection() {
        let (_dir, conn) = scratch("live-terms");
        let file = sources::id(&conn, "file").unwrap();
        let other = sources::id(&conn, "other").unwrap();
        let note = |note_id, term, source, collection| NewNote {
            note_id,
            sentence: "文",
            term: Some(term),
            source: Some(source),
            batch_id: None,
            collection: Some(collection),
            deleted_at: None,
        };
        for n in [
            note(1, "犬", file, "A"),
            note(2, "猫", file, "A"),
            note(3, "鳥", other, "A"),
            note(4, "魚", file, "B"),
        ] {
            insert(&conn, &n).unwrap();
        }
        mark_deleted(&conn, &[2], 10).unwrap();
        assert_eq!(live_terms(&conn, "A", "file").unwrap(), ["犬"]);
        assert_eq!(live_ids(&conn, "A").unwrap(), [1, 3]);
    }
}
