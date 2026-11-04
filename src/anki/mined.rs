//! Already-mined detection (issue #3): terms Yomine mined from the loaded source and
//! sentences that already exist in the user's notes.

use std::collections::{
    HashMap,
    HashSet,
};

use rusqlite::Connection;
use wana_kana::IsJapaneseStr;

use crate::{
    core::utils::{
        normalize_japanese_text,
        FilterKana,
    },
    persistence::db,
};

/// Note-id-keyed so Anki-side deletions can be pruned between harvests.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MinedSentence {
    pub note_id: u64,
    pub sentence: String,
}

/// Match key for a dictionary entry: normalized (expression, reading).
pub fn entry_key(expression: &str, reading: &str) -> String {
    let expression = normalize_japanese_text(strip_html(expression).trim());
    // Anki reading fields carry furigana markup that Yomitan's readings never do.
    let mut reading = normalize_japanese_text(&strip_html(reading).filter_kana());
    // Mirrors the vocab harvest's fallback (sync.rs): a kana term is its own reading.
    if reading.is_empty() && expression.as_str().is_kana() {
        reading = expression.clone();
    }
    format!("{expression}\u{0}{reading}")
}

/// Every (term, reading) in the latest harvested collection as `entry_key`s; empty
/// until one is harvested.
pub fn known_entry_keys() -> HashSet<String> {
    let vocab = db::with(|conn| db::anki::live_cards(conn, &db::anki::active_collection(conn)?))
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            Vec::new()
        });
    vocab.iter().map(|v| entry_key(&v.term, &v.reading)).collect()
}

/// Records a note Yomine created; its sentence counts as mined even without a
/// sentence-field mapping.
pub fn record_note(
    note_id: u64,
    raw_sentence: &str,
    term: &str,
    fingerprint: Option<&str>,
    batch_id: Option<&str>,
    collection: Option<&str>,
) {
    let result = db::with(|conn| {
        let source = fingerprint.map(|f| db::sources::id(conn, f)).transpose()?;
        let note = db::notes::NewNote {
            note_id,
            sentence: &normalize_sentence(raw_sentence),
            term: Some(term),
            source,
            batch_id,
            collection,
            deleted_at: None,
        };
        db::notes::insert(conn, &note)
    });
    if let Err(e) = result {
        eprintln!("Failed to record mined note: {e}");
    }
}

pub fn mark_notes_deleted(note_ids: &[u64]) -> Result<(), String> {
    db::with(|conn| db::notes::mark_deleted(conn, note_ids, db::now_ms()))
}

/// The latest synced collection's sentences, from its notes and from the notes Yomine
/// created, minus notes seen deleted.
pub fn mined_sentences() -> Vec<String> {
    let loaded = db::with(|conn| {
        let collection = db::anki::active_collection(conn)?;
        let mut sentences = db::anki::live_sentences(conn, &collection)?;
        sentences.extend(db::notes::live_sentences(conn, &collection)?);
        Ok(sentences)
    });
    let sentences: HashSet<String> = loaded
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            Vec::new()
        })
        .into_iter()
        .map(|entry| entry.sentence)
        .collect();
    sentences.into_iter().collect()
}

/// Fewest of a source's lines in notes Yomine has no record of for it to count as mined
/// before; a line or two can be in a card mined from another episode.
const MINED_BEFORE_LINES: usize = 3;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MinedBefore {
    pub notes: usize,
    /// The newest note's id, which Anki makes its creation time in ms.
    pub last_note: u64,
}

/// The notes of the latest synced collection Yomine has no record of, by sentence: what
/// mining outside Yomine left.
pub struct Unrecorded(HashMap<String, Vec<u64>>);

impl Unrecorded {
    pub fn load() -> Self {
        db::with(|conn| Self::load_in(conn, &db::anki::active_collection(conn)?)).unwrap_or_else(
            |e| {
                eprintln!("{e}");
                Self(HashMap::new())
            },
        )
    }

    fn load_in(conn: &Connection, collection: &str) -> rusqlite::Result<Self> {
        let recorded: HashSet<u64> = db::notes::ids(conn)?.into_iter().collect();
        let mut notes: HashMap<String, Vec<u64>> = HashMap::new();
        for note in db::anki::live_sentences(conn, collection)? {
            if !recorded.contains(&note.note_id) {
                notes.entry(note.sentence).or_default().push(note.note_id);
            }
        }
        Ok(Self(notes))
    }

    /// The notes holding one of a source's `lines`, when enough lines are in them to say it
    /// was mined before; none otherwise.
    pub fn mined_from<'a>(&'a self, lines: &[&str]) -> Vec<(&'a str, u64)> {
        let lines: HashSet<String> =
            lines.iter().map(|s| normalize_sentence(s)).filter(|s| !s.is_empty()).collect();
        let found: Vec<(&str, &Vec<u64>)> = self
            .0
            .iter()
            .filter(|(sentence, _)| lines.contains(*sentence))
            .map(|(sentence, ids)| (sentence.as_str(), ids))
            .collect();
        if found.len() < MINED_BEFORE_LINES {
            return Vec::new();
        }
        found
            .into_iter()
            .flat_map(|(sentence, ids)| ids.iter().map(move |&id| (sentence, id)))
            .collect()
    }
}

/// The notes found already mined from the source `fingerprint` outside Yomine, recording
/// first the ones its `lines` show it was.
pub fn mined_before(fingerprint: &str, lines: &[&str]) -> Option<MinedBefore> {
    let found = db::with(|conn| {
        let collection = db::anki::active_collection(conn)?;
        let unrecorded = Unrecorded::load_in(conn, &collection)?;
        let tx = conn.transaction()?;
        let source = db::sources::id(&tx, fingerprint)?;
        for (sentence, note_id) in unrecorded.mined_from(lines) {
            db::notes::insert(
                &tx,
                &db::notes::NewNote {
                    note_id,
                    sentence,
                    term: None,
                    source: Some(source),
                    batch_id: None,
                    collection: Some(&collection),
                    deleted_at: None,
                },
            )?;
        }
        let found = db::notes::found_mined(&tx, &collection, source)?;
        tx.commit()?;
        Ok(found.map(|(notes, last_note)| MinedBefore { notes, last_note }))
    });
    found.unwrap_or_else(|e| {
        eprintln!("Failed to check for notes mined before: {e}");
        None
    })
}

/// Terms Yomine mined from a source into the latest synced collection, minus notes seen
/// deleted.
pub fn mined_terms(fingerprint: &str) -> Vec<String> {
    db::with(|conn| db::notes::live_terms(conn, &db::anki::active_collection(conn)?, fingerprint))
        .unwrap_or_else(|e| {
            eprintln!("{e}");
            Vec::new()
        })
}

/// Sentence match key: tags stripped, whitespace removed. Must stay in sync
/// with the frontend's `normalizeSentence`.
pub fn normalize_sentence(raw: &str) -> String {
    strip_html(raw).chars().filter(|c| !c.is_whitespace()).collect()
}

pub fn strip_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_nbsp() {
        assert_eq!(strip_html("毎日<b>パン</b>を&nbsp;食べる"), "毎日パンを 食べる");
        assert_eq!(strip_html("no tags"), "no tags");
    }

    #[test]
    fn normalizes_whitespace_including_fullwidth() {
        assert_eq!(
            normalize_sentence("毎日\u{3000}パンを <b>食べる</b>。\n"),
            "毎日パンを食べる。"
        );
    }

    #[test]
    fn entries_sharing_an_expression_key_apart() {
        assert_ne!(entry_key("期", "き"), entry_key("期", "ご"));
    }

    #[test]
    fn note_markup_keys_like_a_clean_entry() {
        assert_eq!(entry_key("<b>期</b>", "期[き]"), entry_key("期", "き"));
    }

    #[test]
    fn katakana_and_hiragana_readings_agree() {
        assert_eq!(entry_key("チャンス", "チャンス"), entry_key("ちゃんす", "ちゃんす"));
    }

    #[test]
    fn long_vowel_spellings_fold_together() {
        assert_eq!(entry_key("遠い", "とおい"), entry_key("遠い", "とうい"));
    }

    #[test]
    fn kana_note_without_a_reading_keys_like_the_entry() {
        assert_eq!(entry_key("チャンス", ""), entry_key("チャンス", "チャンス"));
    }

    #[test]
    fn ruby_furigana_readings_survive() {
        // Anki sentence fields often carry ruby markup; tags go, text stays.
        assert_eq!(normalize_sentence("<ruby>食<rt>た</rt></ruby>べる"), "食たべる");
    }

    #[test]
    fn a_source_is_mined_before_from_three_lines_in_notes() {
        let unrecorded = Unrecorded(HashMap::from([
            ("猫だ".to_string(), vec![1]),
            ("犬だ".to_string(), vec![2, 3]),
            ("鳥だ".to_string(), vec![4]),
        ]));
        assert!(unrecorded.mined_from(&["猫だ", "犬 だ", "魚だ"]).is_empty());
        let mut notes: Vec<u64> =
            unrecorded.mined_from(&["猫だ", "犬 だ", "<b>鳥</b>だ"]).iter().map(|n| n.1).collect();
        notes.sort();
        assert_eq!(notes, [1, 2, 3, 4]);
    }
}
