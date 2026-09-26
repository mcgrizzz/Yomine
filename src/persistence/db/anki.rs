//! Anki's cards and sentence fields, per collection, as harvests saw them.

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
    conn.query_row("SELECT value FROM meta WHERE key = 'anki_collection'", [], |r| r.get(0))
        .optional()
        .map(|c| c.unwrap_or_else(|| UNKNOWN.to_string()))
}

fn set_active_collection(conn: &Connection, collection: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('anki_collection', ?1)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![collection],
    )
    .map(|_| ())
}

/// Hands the imported rows to the first collection harvested.
fn claim_unknown(conn: &Connection, collection: &str) -> rusqlite::Result<()> {
    for table in ["anki_cards", "anki_card_history", "anki_sentences"] {
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

pub(super) fn insert_card(
    conn: &Connection,
    collection: &str,
    card_id: u64,
    vocab: &Vocab,
    now: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO anki_cards
             (collection, card_id, term, reading, interval, first_seen_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![collection, card_id as i64, vocab.term, vocab.reading, vocab.interval, now],
    )?;
    record_history(conn, collection, card_id, vocab.interval, now)
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

/// Brings the collection's cards in line with a harvest: new cards are added, changed
/// ones updated (a changed state also logged), and missing ones marked gone.
pub fn sync_cards(
    conn: &mut Connection,
    collection: &str,
    vocab: &[Vocab],
    now: i64,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    set_active_collection(&tx, collection)?;
    claim_unknown(&tx, collection)?;
    let existing: HashMap<u64, (String, String, Option<f32>, bool)> = tx
        .prepare(
            "SELECT card_id, term, reading, interval, gone_at IS NOT NULL
             FROM anki_cards WHERE collection = ?1",
        )?
        .query_map(params![collection], |r| {
            Ok((r.get::<_, i64>(0)? as u64, (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut seen = HashSet::new();
    for card in vocab {
        // Every note has a card; the harvest leaves this empty only for a malformed one.
        let Some(card_id) = card.card_id else { continue };
        seen.insert(card_id);
        let Some((term, reading, interval, gone)) = existing.get(&card_id) else {
            insert_card(&tx, collection, card_id, card, now)?;
            continue;
        };
        let interval_changed = *interval != card.interval;
        if *term != card.term || *reading != card.reading || interval_changed || *gone {
            tx.execute(
                "UPDATE anki_cards SET term = ?3, reading = ?4, interval = ?5, updated_at = ?6,
                     gone_at = NULL
                 WHERE collection = ?1 AND card_id = ?2",
                params![collection, card_id as i64, card.term, card.reading, card.interval, now],
            )?;
        }
        if interval_changed {
            record_history(&tx, collection, card_id, card.interval, now)?;
        }
    }
    for (card_id, (.., gone)) in &existing {
        if !gone && !seen.contains(card_id) {
            tx.execute(
                "UPDATE anki_cards SET gone_at = ?3 WHERE collection = ?1 AND card_id = ?2",
                params![collection, *card_id as i64, now],
            )?;
        }
    }
    tx.commit()
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

/// Brings the collection's sentence fields in line with a harvest, like `sync_cards`.
pub fn sync_sentences(
    conn: &mut Connection,
    collection: &str,
    sentences: &[MinedSentence],
    now: i64,
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    claim_unknown(&tx, collection)?;
    let existing: HashMap<u64, (String, bool)> = tx
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
            None => insert_sentence(&tx, collection, sentence, now)?,
            Some((text, gone)) if *text != sentence.sentence || *gone => {
                tx.execute(
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
        .filter(|(id, (_, gone))| !gone && !seen.contains(id))
        .map(|(id, _)| *id)
        .collect();
    mark_sentences_gone(&tx, collection, &gone, now)?;
    tx.commit()
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

    #[test]
    fn harvests_date_new_changed_and_missing_cards_per_collection() {
        let (dir, mut conn) = scratch("anki");
        sync_cards(&mut conn, "Main", &[card(1, "猫", 1.0), card(2, "犬", 3.0)], 10).unwrap();
        sync_cards(&mut conn, "Main", &[card(1, "猫", 4.0)], 20).unwrap();
        sync_cards(&mut conn, "Other", &[card(1, "鳥", 1.0)], 30).unwrap();

        let main = live_cards(&conn, "Main").unwrap();
        assert_eq!(main.len(), 1);
        assert_eq!(main[0].interval, Some(4.0));
        assert_eq!(
            count(
                &conn,
                "SELECT gone_at FROM anki_cards WHERE collection = 'Main' AND card_id = 2"
            ),
            20
        );
        assert_eq!(
            count(
                &conn,
                "SELECT first_seen_at FROM anki_cards WHERE collection = 'Main' AND card_id = 1"
            ),
            10
        );
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM anki_card_history WHERE collection = 'Main' AND card_id = 1"
            ),
            2
        );
        assert_eq!(live_cards(&conn, "Other").unwrap()[0].term, "鳥");
        assert_eq!(active_collection(&conn).unwrap(), "Other");

        sync_cards(&mut conn, "Main", &[card(1, "猫", 4.0), card(2, "犬", 3.0)], 40).unwrap();
        assert_eq!(live_cards(&conn, "Main").unwrap().len(), 2);
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
