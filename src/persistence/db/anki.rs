//! Anki's notes, cards and sentence fields, per collection, as harvests saw them.

use std::collections::{
    HashMap,
    HashSet,
};

use rusqlite::{
    params,
    Connection,
    OptionalExtension,
};

use crate::anki::{
    mined::MinedSentence,
    types::Vocab,
};

/// Rows imported from the JSON caches, before any harvest named their collection.
pub(super) const UNKNOWN: &str = "";

/// The collection the latest harvest came from, so an offline start reads its rows.
pub fn active_collection(conn: &Connection) -> rusqlite::Result<String> {
    meta(conn, "anki_collection").map(|c| c.unwrap_or_else(|| UNKNOWN.to_string()))
}

fn meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| r.get(0)).optional()
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map(|_| ())
}

/// What the next harvest of a collection compares against.
pub struct HarvestState {
    /// Notes the collection had at its last harvest.
    pub notes: HashSet<u64>,
    pub harvested_at: Option<i64>,
    /// The note-type mapping that harvest read fields with.
    pub mapping: Option<String>,
}

pub fn harvest_state(conn: &Connection, collection: &str) -> rusqlite::Result<HarvestState> {
    Ok(HarvestState {
        notes: conn
            .prepare("SELECT note_id FROM anki_notes WHERE collection = ?1 AND gone_at IS NULL")?
            .query_map(params![collection], |r| r.get::<_, i64>(0).map(|id| id as u64))?
            .collect::<rusqlite::Result<_>>()?,
        harvested_at: meta(conn, &format!("harvested_at:{collection}"))?
            .and_then(|at| at.parse().ok()),
        mapping: meta(conn, &format!("mapping:{collection}"))?,
    })
}

/// One harvest's view of a collection.
pub struct Harvest<'a> {
    /// Every note the collection has now.
    pub note_ids: &'a [u64],
    /// Notes read in full this time; the others are unchanged since the last harvest.
    pub fetched: &'a [u64],
    /// Vocab cards of the fetched notes, by note id.
    pub cards: &'a [(u64, Vocab)],
    /// Sentence fields of the fetched notes.
    pub sentences: &'a [MinedSentence],
    /// Every note was fetched; rows the JSON import left without a note id settle then.
    pub full: bool,
    pub mapping: &'a str,
}

/// Hands the imported rows to the first collection harvested.
fn claim_unknown(conn: &Connection, collection: &str) -> rusqlite::Result<()> {
    for table in ["anki_notes", "anki_cards", "anki_card_history", "anki_sentences"] {
        conn.execute(
            &format!(
                "UPDATE {table} SET collection = ?1 WHERE collection = ?2
                 AND NOT EXISTS (SELECT 1 FROM {table} WHERE collection = ?1)"
            ),
            params![collection, UNKNOWN],
        )?;
    }
    Ok(())
}

/// Brings the collection in line with a harvest: new rows are added and dated, changed
/// ones updated (a changed card state also logged), and missing ones marked gone.
pub fn apply_harvest(
    conn: &mut Connection,
    collection: &str,
    harvest: &Harvest,
    now: i64,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    set_meta(&tx, "anki_collection", collection)?;
    claim_unknown(&tx, collection)?;

    let present: HashSet<u64> = harvest.note_ids.iter().copied().collect();
    let known: HashMap<u64, bool> = tx
        .prepare("SELECT note_id, gone_at IS NOT NULL FROM anki_notes WHERE collection = ?1")?
        .query_map(params![collection], |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for id in &present {
        match known.get(id) {
            None => {
                tx.execute(
                    "INSERT INTO anki_notes (collection, note_id, first_seen_at)
                     VALUES (?1, ?2, ?3)",
                    params![collection, *id as i64, now],
                )?;
            }
            Some(true) => {
                tx.execute(
                    "UPDATE anki_notes SET gone_at = NULL WHERE collection = ?1 AND note_id = ?2",
                    params![collection, *id as i64],
                )?;
            }
            Some(false) => {}
        }
    }
    let removed: HashSet<u64> = known
        .iter()
        .filter(|(id, gone)| !**gone && !present.contains(id))
        .map(|(id, _)| *id)
        .collect();
    for id in &removed {
        tx.execute(
            "UPDATE anki_notes SET gone_at = ?3 WHERE collection = ?1 AND note_id = ?2",
            params![collection, *id as i64, now],
        )?;
    }

    let fetched: HashSet<u64> = harvest.fetched.iter().copied().collect();
    // A row is settled by this harvest when its note was read or deleted.
    let settled = |note: Option<u64>| match note {
        Some(note) => fetched.contains(&note) || removed.contains(&note),
        None => harvest.full,
    };
    sync_cards(&tx, collection, harvest, &settled, now)?;
    sync_sentences(&tx, collection, harvest.sentences, &settled, now)?;

    set_meta(&tx, &format!("harvested_at:{collection}"), &now.to_string())?;
    set_meta(&tx, &format!("mapping:{collection}"), harvest.mapping)?;
    tx.commit()
}

type StoredCard = (Option<u64>, String, String, Option<f32>, bool);

fn sync_cards(
    conn: &Connection,
    collection: &str,
    harvest: &Harvest,
    settled: &dyn Fn(Option<u64>) -> bool,
    now: i64,
) -> rusqlite::Result<()> {
    let existing: HashMap<u64, StoredCard> = conn
        .prepare(
            "SELECT card_id, note_id, term, reading, interval, gone_at IS NOT NULL
             FROM anki_cards WHERE collection = ?1",
        )?
        .query_map(params![collection], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                (
                    r.get::<_, Option<i64>>(1)?.map(|id| id as u64),
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ),
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut seen = HashSet::new();
    for (note_id, card) in harvest.cards {
        // Every note has a card; the harvest leaves this empty only for a malformed one.
        let Some(card_id) = card.card_id else { continue };
        seen.insert(card_id);
        let Some((stored_note, term, reading, interval, gone)) = existing.get(&card_id) else {
            insert_card(conn, collection, card_id, Some(*note_id), card, now)?;
            continue;
        };
        if *stored_note != Some(*note_id)
            || *term != card.term
            || *reading != card.reading
            || *interval != card.interval
            || *gone
        {
            conn.execute(
                "UPDATE anki_cards SET note_id = ?3, term = ?4, reading = ?5, interval = ?6,
                     updated_at = ?7, gone_at = NULL
                 WHERE collection = ?1 AND card_id = ?2",
                params![
                    collection,
                    card_id as i64,
                    *note_id as i64,
                    card.term,
                    card.reading,
                    card.interval,
                    now
                ],
            )?;
        }
        if *interval != card.interval {
            record_history(conn, collection, card_id, card.interval, now)?;
        }
    }
    for (card_id, (note, .., gone)) in &existing {
        if !gone && !seen.contains(card_id) && settled(*note) {
            conn.execute(
                "UPDATE anki_cards SET gone_at = ?3 WHERE collection = ?1 AND card_id = ?2",
                params![collection, *card_id as i64, now],
            )?;
        }
    }
    Ok(())
}

fn sync_sentences(
    conn: &Connection,
    collection: &str,
    sentences: &[MinedSentence],
    settled: &dyn Fn(Option<u64>) -> bool,
    now: i64,
) -> rusqlite::Result<()> {
    let existing: HashMap<u64, (String, bool)> = conn
        .prepare(
            "SELECT note_id, sentence, gone_at IS NOT NULL FROM anki_sentences
             WHERE collection = ?1",
        )?
        .query_map(params![collection], |r| {
            Ok((r.get::<_, i64>(0)? as u64, (r.get(1)?, r.get(2)?)))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut seen = HashSet::new();
    for sentence in sentences {
        seen.insert(sentence.note_id);
        match existing.get(&sentence.note_id) {
            None => insert_sentence(conn, collection, sentence, now)?,
            Some((text, gone)) if *text != sentence.sentence || *gone => {
                conn.execute(
                    "UPDATE anki_sentences SET sentence = ?3, gone_at = NULL
                     WHERE collection = ?1 AND note_id = ?2",
                    params![collection, sentence.note_id as i64, sentence.sentence],
                )?;
            }
            Some(_) => {}
        }
    }
    let gone: Vec<u64> = existing
        .iter()
        .filter(|(id, (_, gone))| !gone && !seen.contains(id) && settled(Some(**id)))
        .map(|(id, _)| *id)
        .collect();
    mark_sentences_gone(conn, collection, &gone, now)
}

pub(super) fn insert_card(
    conn: &Connection,
    collection: &str,
    card_id: u64,
    note_id: Option<u64>,
    card: &Vocab,
    now: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO anki_cards
             (collection, card_id, note_id, term, reading, interval, first_seen_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            collection,
            card_id as i64,
            note_id.map(|id| id as i64),
            card.term,
            card.reading,
            card.interval,
            now
        ],
    )?;
    record_history(conn, collection, card_id, card.interval, now)
}

fn record_history(
    conn: &Connection,
    collection: &str,
    card_id: u64,
    interval: Option<f32>,
    now: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO anki_card_history (collection, card_id, at, interval) VALUES (?1, ?2, ?3, ?4)",
        params![collection, card_id as i64, now, interval],
    )
    .map(|_| ())
}

/// The collection's cards still in Anki, as the latest harvest saw them.
pub fn live_cards(conn: &Connection, collection: &str) -> rusqlite::Result<Vec<Vocab>> {
    conn.prepare(
        "SELECT term, reading, card_id, interval FROM anki_cards
         WHERE collection = ?1 AND gone_at IS NULL",
    )?
    .query_map(params![collection], |r| {
        Ok(Vocab {
            term: r.get(0)?,
            reading: r.get(1)?,
            card_id: Some(r.get::<_, i64>(2)? as u64),
            interval: r.get(3)?,
        })
    })?
    .collect()
}

pub(super) fn insert_sentence(
    conn: &Connection,
    collection: &str,
    sentence: &MinedSentence,
    now: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO anki_sentences (collection, note_id, sentence, first_seen_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![collection, sentence.note_id as i64, sentence.sentence, now],
    )
    .map(|_| ())
}

pub fn mark_sentences_gone(
    conn: &Connection,
    collection: &str,
    note_ids: &[u64],
    now: i64,
) -> rusqlite::Result<()> {
    for id in note_ids {
        conn.execute(
            "UPDATE anki_sentences SET gone_at = ?3
             WHERE collection = ?1 AND note_id = ?2 AND gone_at IS NULL",
            params![collection, *id as i64, now],
        )?;
    }
    Ok(())
}

pub fn live_sentences(conn: &Connection, collection: &str) -> rusqlite::Result<Vec<MinedSentence>> {
    conn.prepare(
        "SELECT note_id, sentence FROM anki_sentences WHERE collection = ?1 AND gone_at IS NULL",
    )?
    .query_map(params![collection], |r| {
        Ok(MinedSentence { note_id: r.get::<_, i64>(0)? as u64, sentence: r.get(1)? })
    })?
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        super::tests::{
            count,
            scratch,
        },
        *,
    };

    fn card(id: u64, term: &str, interval: f32) -> Vocab {
        Vocab {
            term: term.into(),
            reading: "よみ".into(),
            card_id: Some(id),
            interval: Some(interval),
        }
    }

    fn harvest<'a>(
        note_ids: &'a [u64],
        fetched: &'a [u64],
        cards: &'a [(u64, Vocab)],
    ) -> Harvest<'a> {
        Harvest {
            note_ids,
            fetched,
            cards,
            sentences: &[],
            full: note_ids == fetched,
            mapping: "{}",
        }
    }

    #[test]
    fn incremental_harvests_keep_unfetched_cards_and_retire_deleted_notes() {
        let (dir, mut conn) = scratch("anki");
        let all = [10, 20, 30, 40];
        let cards = [(10, card(1, "猫", 1.0)), (20, card(2, "犬", 3.0)), (30, card(3, "鳥", 2.0))];
        apply_harvest(&mut conn, "Main", &harvest(&all, &all, &cards), 1).unwrap();

        // Note 20 was deleted and 30 edited into a non-vocab note; 10 wasn't fetched.
        apply_harvest(&mut conn, "Main", &harvest(&[10, 30, 40], &[30], &[]), 2).unwrap();

        let live = live_cards(&conn, "Main").unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].interval, Some(1.0));
        assert_eq!(count(&conn, "SELECT gone_at FROM anki_cards WHERE card_id = 2"), 2);
        assert_eq!(count(&conn, "SELECT gone_at FROM anki_cards WHERE card_id = 3"), 2);
        assert_eq!(count(&conn, "SELECT gone_at FROM anki_notes WHERE note_id = 20"), 2);

        // A full harvest picks up the review the incremental one skipped.
        let now = [10, 30, 40];
        apply_harvest(&mut conn, "Main", &harvest(&now, &now, &[(10, card(1, "猫", 9.0))]), 3)
            .unwrap();
        assert_eq!(live_cards(&conn, "Main").unwrap()[0].interval, Some(9.0));
        assert_eq!(count(&conn, "SELECT count(*) FROM anki_card_history WHERE card_id = 1"), 2);

        let state = harvest_state(&conn, "Main").unwrap();
        assert_eq!(state.notes, HashSet::from([10, 30, 40]));
        assert_eq!(state.harvested_at, Some(3));
        assert!(harvest_state(&conn, "Other").unwrap().notes.is_empty());
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
