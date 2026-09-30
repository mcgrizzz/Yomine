//! Keeps Yomine's copy of the open Anki collection (`db::anki`) in step with Anki.

use std::{
    collections::{
        BTreeMap,
        HashMap,
        HashSet,
    },
    sync::Mutex,
    time::Instant,
};

use rayon::iter::{
    IntoParallelIterator,
    ParallelIterator,
};
use wana_kana::IsJapaneseStr;

use super::{
    client::{
        AnkiError,
        NoteInfo,
    },
    types::{
        FieldMapping,
        Vocab,
    },
};
use crate::{
    core::utils::{
        FilterKana,
        NormalizeLongVowel,
    },
    persistence::db,
};

static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Reads the notes added or edited in the open collection since the last sync, and marks
/// deleted ones gone, including notes Yomine recorded creating. Returns whether the copy
/// changed. One sync runs at a time.
pub async fn sync(model_mapping: &HashMap<String, FieldMapping>) -> Result<bool, AnkiError> {
    let _running = RUNNING.lock().await;
    harvest(model_mapping).await.inspect_err(|_| anki_unreachable())
}

/// Collections harvested in full since Yomine started or Anki last stopped answering.
/// Their next harvests fetch only new and edited notes; intervals refresh with a full one.
static CURRENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Anki stopped answering, so anything may have changed: the next harvests are full.
pub(crate) fn anki_unreachable() {
    CURRENT.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

async fn harvest(model_mapping: &HashMap<String, FieldMapping>) -> Result<bool, AnkiError> {
    let start = Instant::now();
    let anki = super::current();
    let collection = anki.profile().await?;
    let mapping = serde_json::to_string(&model_mapping.iter().collect::<BTreeMap<_, _>>())
        .expect("field mappings are plain data");
    let note_ids = anki.find_notes("deck:*").await?;
    let now = db::now_ms();

    let current = CURRENT.lock().unwrap_or_else(|e| e.into_inner()).contains(&collection);
    let stored = db::with(|conn| db::anki::harvest_state(conn, &collection))
        .inspect_err(|e| eprintln!("{e}"))
        .ok()
        .filter(|s| {
            current && s.mapping.as_deref() == Some(mapping.as_str()) && !s.notes.is_empty()
        });
    let fetch: Vec<u64> = match (&stored, stored.as_ref().and_then(|s| s.harvested_at)) {
        (Some(stored), Some(harvested_at)) => {
            // edited:N counts back from the start of today, so one extra day covers a
            // harvest late yesterday.
            let days = (now - harvested_at) / 86_400_000 + 1;
            let edited: HashSet<u64> =
                anki.find_notes(&format!("edited:{days}")).await?.into_iter().collect();
            note_ids
                .iter()
                .copied()
                .filter(|id| !stored.notes.contains(id) || edited.contains(id))
                .collect()
        }
        _ => note_ids.clone(),
    };
    let full = fetch.len() == note_ids.len();

    let (mut cards, sentences) = read_notes(model_mapping, anki.notes(&fetch).await?);
    let card_ids: Vec<u64> = cards.iter().filter_map(|(_, card)| card.card_id).collect();
    let intervals: HashMap<u64, i32> =
        card_ids.iter().copied().zip(anki.intervals(&card_ids).await?).collect();
    for (_, card) in &mut cards {
        // Negative intervals are in seconds (learning/relearning), positive in days.
        card.interval = card.card_id.and_then(|id| intervals.get(&id)).map(|&interval| {
            if interval >= 0 {
                interval as f32
            } else {
                interval.unsigned_abs() as f32 / 86400.0
            }
        });
    }
    println!(
        "Harvested {} of {} notes from Anki profile {collection:?} ({:.1}s)",
        fetch.len(),
        note_ids.len(),
        start.elapsed().as_secs_f32()
    );

    let present: HashSet<u64> = note_ids.iter().copied().collect();
    let removed = stored.as_ref().is_some_and(|s| s.notes.iter().any(|id| !present.contains(id)));
    let recorded_gone = mark_recorded_gone(&collection, &present, now);

    if full && cards.is_empty() {
        eprintln!("Anki returned no vocab; keeping the existing cache");
        return Ok(recorded_gone);
    }
    let harvest = db::anki::Harvest {
        note_ids: &note_ids,
        fetched: &fetch,
        cards: &cards,
        sentences: &sentences,
        full,
        mapping: &mapping,
    };
    match db::with(|conn| db::anki::apply_harvest(conn, &collection, &harvest, now)) {
        Ok(()) if full => CURRENT.lock().unwrap_or_else(|e| e.into_inner()).push(collection),
        Ok(()) => {}
        Err(e) => {
            eprintln!("Failed to save Anki vocab cache: {e}");
            return Ok(recorded_gone);
        }
    }
    Ok(!fetch.is_empty() || removed || recorded_gone)
}

/// Marks the notes Yomine created in `collection` deleted when Anki no longer has them.
fn mark_recorded_gone(collection: &str, present: &HashSet<u64>, now: i64) -> bool {
    let result = db::with(|conn| {
        let gone: Vec<u64> = db::notes::live_ids(conn, collection)?
            .into_iter()
            .filter(|id| !present.contains(id))
            .collect();
        db::notes::mark_deleted(conn, &gone, now)?;
        Ok(!gone.is_empty())
    });
    result.unwrap_or_else(|e| {
        eprintln!("Failed to mark deleted notes: {e}");
        false
    })
}

/// Vocab cards, by note id, and sentence-field values (issue #3) of the given notes.
fn read_notes(
    model_mapping: &HashMap<String, FieldMapping>,
    notes: Vec<NoteInfo>,
) -> (Vec<(u64, Vocab)>, Vec<super::mined::MinedSentence>) {
    let sentences = notes
        .iter()
        .filter_map(|note| {
            let mapping = model_mapping.get(&note.note_type)?;
            let sentence_field = mapping.sentence_field.as_ref()?;
            let value = note.fields.get(sentence_field)?;
            let normalized = super::mined::normalize_sentence(value);
            (!normalized.is_empty())
                .then_some(super::mined::MinedSentence { note_id: note.id, sentence: normalized })
        })
        .collect();

    let cards = notes
        .into_par_iter()
        .filter_map(|note| {
            let field_mapping = model_mapping.get(&note.note_type)?;
            let term = note.fields.get(&field_mapping.term_field)?.clone();
            let mut reading = note.fields.get(&field_mapping.reading_field)?.clone();
            if reading.trim().is_empty() && term.as_str().is_kana() {
                reading = term.clone();
            }
            Some((
                note.id,
                Vocab {
                    term,
                    reading: reading.filter_kana().normalize_long_vowel().into_owned(),
                    card_id: note.cards.first().copied(),
                    interval: None,
                },
            ))
        })
        .collect();
    (cards, sentences)
}
