//! Already-mined detection (issue #3): terms with a recently-added card and
//! sentences that already exist in the user's notes.

use std::collections::HashMap;

use super::{
    api::{
        get_note_ids,
        get_notes,
    },
    types::FieldMapping,
};

/// Sentence-field harvest, overwritten during the full note pass in
/// `get_total_vocab`; read (and pruned) by `get_mined_state`.
pub const MINED_SENTENCE_CACHE: &str = "anki_mined_sentences.json";

/// Sentences Yomine itself mined — survives without a sentence-field mapping.
const RECORDED_MINES_CACHE: &str = "yomine_mined_notes.json";

/// Both caches key sentences by note id so Anki-side deletions can be pruned
/// between full harvests.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MinedSentence {
    pub note_id: u64,
    pub sentence: String,
}

/// Terms + sentences from notes added in the last day (`added:1`). Term fields
/// are tag-stripped/trimmed; sentences are normalized for exact matching.
pub async fn get_recently_added(
    model_mapping: &HashMap<String, FieldMapping>,
) -> Result<(Vec<String>, Vec<String>), reqwest::Error> {
    let note_ids = get_note_ids("added:1").await?;
    if note_ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let notes = get_notes(note_ids).await?;

    let mut terms = Vec::new();
    let mut sentences = Vec::new();
    for note in notes {
        let Some(mapping) = model_mapping.get(&note.model_name) else { continue };
        if let Some(field) = note.fields.get(&mapping.term_field) {
            let term = strip_html(&field.value).trim().to_string();
            if !term.is_empty() {
                terms.push(term);
            }
        }
        if let Some(sentence_field) = &mapping.sentence_field {
            if let Some(field) = note.fields.get(sentence_field) {
                let sentence = normalize_sentence(&field.value);
                if !sentence.is_empty() {
                    sentences.push(sentence);
                }
            }
        }
    }
    Ok((terms, sentences))
}

/// Written by the `get_total_vocab` harvest.
pub fn save_harvested_sentences(entries: &[MinedSentence]) {
    if let Err(e) = crate::persistence::save_json(&entries, MINED_SENTENCE_CACHE) {
        eprintln!("Failed to save mined sentence cache: {}", e);
    }
}

/// Record a sentence Yomine itself just mined, so "sentence mined" marks
/// survive restarts even without a sentence-field mapping.
pub fn record_mined_sentence(note_id: u64, raw: &str) {
    let normalized = normalize_sentence(raw);
    if normalized.is_empty() {
        return;
    }
    let mut recorded: Vec<MinedSentence> =
        crate::persistence::load_json_or_default(RECORDED_MINES_CACHE);
    if !recorded.iter().any(|r| r.note_id == note_id) {
        recorded.push(MinedSentence { note_id, sentence: normalized });
        if let Err(e) = crate::persistence::save_json(&recorded, RECORDED_MINES_CACHE) {
            eprintln!("Failed to update recorded mines cache: {}", e);
        }
    }
}

/// Note ids that still exist in Anki, queried in chunks (`nid:a,b,c`). `None`
/// when Anki is unreachable — callers keep their caches as-is.
async fn existing_note_ids(ids: &[u64]) -> Option<std::collections::HashSet<u64>> {
    let mut existing = std::collections::HashSet::new();
    for chunk in ids.chunks(500) {
        let query =
            format!("nid:{}", chunk.iter().map(u64::to_string).collect::<Vec<_>>().join(","));
        match get_note_ids(&query).await {
            Ok(found) => existing.extend(found),
            Err(_) => return None,
        }
    }
    Some(existing)
}

/// All mined-sentence match keys (harvest + Yomine's own mines), minus notes
/// since deleted in Anki — both caches are pruned in passing, so a deleted
/// note's sentence unmarks on the next refresh instead of the next full
/// harvest. Offline Anki returns the caches untouched.
pub async fn mined_sentences_pruned() -> Vec<String> {
    let mut harvested: Vec<MinedSentence> =
        crate::persistence::load_json_or_default(MINED_SENTENCE_CACHE);
    let mut recorded: Vec<MinedSentence> =
        crate::persistence::load_json_or_default(RECORDED_MINES_CACHE);

    let ids: Vec<u64> =
        harvested.iter().chain(recorded.iter()).map(|entry| entry.note_id).collect();
    if !ids.is_empty() {
        if let Some(existing) = existing_note_ids(&ids).await {
            let before = (harvested.len(), recorded.len());
            harvested.retain(|entry| existing.contains(&entry.note_id));
            recorded.retain(|entry| existing.contains(&entry.note_id));
            if harvested.len() != before.0 {
                save_harvested_sentences(&harvested);
            }
            if recorded.len() != before.1 {
                if let Err(e) = crate::persistence::save_json(&recorded, RECORDED_MINES_CACHE) {
                    eprintln!("Failed to prune recorded mines cache: {}", e);
                }
            }
        }
    }

    let sentences: std::collections::HashSet<String> =
        harvested.into_iter().chain(recorded).map(|entry| entry.sentence).collect();
    sentences.into_iter().collect()
}

/// Match key for sentence comparison: tags stripped, all whitespace removed.
/// The frontend applies the same whitespace rule to its (plain-text) sentences,
/// so the two sides must stay in sync.
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
    fn ruby_furigana_readings_survive() {
        // Anki sentence fields often carry ruby markup; tags go, text stays.
        assert_eq!(normalize_sentence("<ruby>食<rt>た</rt></ruby>べる"), "食たべる");
    }
}
