//! Heuristic term/reading field guessing for the Anki model-mapping UI.
//!
//! Moved out of `gui::settings::components` (T040) so it is UI-neutral: both the
//! egui modal and the Tauri `get_anki_sample_note` command call this one
//! implementation (Constitution: single source of truth).

use wana_kana::{
    utils::{
        is_char_kana,
        is_char_kanji,
    },
    IsJapaneseChar,
};

/// Sentence-field guess (issue #3), in priority order: a field literally named
/// "Sentence" (space/case-insensitive), the shortest sentence-named field that
/// isn't a derived variant (audio/furigana/meaning/…), a context-ish name, and
/// finally the first field whose sample content looks like a Japanese sentence.
pub fn guess_sentence_field(
    sample_note: &std::collections::HashMap<String, String>,
    available_fields: &[String],
) -> Option<String> {
    fn norm(s: &str) -> String {
        s.to_lowercase().replace([' ', '_', '-'], "")
    }
    // "SentenceAudio", "Sentence Meaning", "IsSentenceCard" etc. are metadata
    // ABOUT the sentence, not the sentence text itself.
    const EXCLUDED: &[&str] = &[
        "audio",
        "furigana",
        "meaning",
        "reading",
        "translation",
        "english",
        "picture",
        "image",
        "screenshot",
        "hint",
        "card",
        "kana",
        "notes",
    ];
    let excluded = |l: &str| EXCLUDED.iter().any(|x| l.contains(x));

    if let Some(f) = available_fields.iter().find(|f| norm(f) == "sentence") {
        return Some(f.clone());
    }
    let mut candidates: Vec<&String> = available_fields
        .iter()
        .filter(|f| {
            let l = norm(f);
            l.contains("sentence") && !excluded(&l)
        })
        .collect();
    candidates.sort_by_key(|f| f.chars().count());
    if let Some(f) = candidates.first() {
        return Some((*f).clone());
    }
    if let Some(f) = available_fields.iter().find(|f| {
        let l = norm(f);
        (l.contains("context") || l.contains("例文")) && !excluded(&l)
    }) {
        return Some(f.clone());
    }
    available_fields
        .iter()
        .find(|f| sample_note.get(*f).is_some_and(|v| is_likely_sentence(v)))
        .cloned()
}

fn is_likely_sentence(value: &str) -> bool {
    let stripped = super::mined::strip_html(value);
    let trimmed = stripped.trim();
    let chars = trimmed.chars().count();
    chars >= 8
        && trimmed.chars().any(|c| c.is_japanese())
        && (trimmed.contains(['。', '！', '？']) || chars >= 14)
}

pub fn guess_field_mappings(
    sample_note: &std::collections::HashMap<String, String>,
    available_fields: &[String],
) -> (Option<String>, Option<String>) {
    let mut best_term = None;
    let mut best_reading = None;
    let mut term_index = None;

    for (field_index, field_name) in available_fields.iter().enumerate() {
        if let Some(field_value) = sample_note.get(field_name) {
            let trimmed_value = field_value.trim();
            if trimmed_value.is_empty() {
                continue;
            }

            if is_likely_term(trimmed_value) {
                if trimmed_value.chars().any(|c| is_char_kanji(c)) {
                    best_term = Some(field_name.clone());
                    term_index = Some(field_index);
                    break;
                } else if best_term.is_none() {
                    best_term = Some(field_name.clone());
                    term_index = Some(field_index);
                }
            }
        }
    }

    for (field_index, field_name) in available_fields.iter().enumerate() {
        if let Some(field_value) = sample_note.get(field_name) {
            let trimmed_value = field_value.trim();
            if trimmed_value.is_empty() {
                continue;
            }

            if is_likely_reading(trimmed_value) {
                if let Some(term_idx) = term_index {
                    if field_index > term_idx {
                        best_reading = Some(field_name.clone());
                        break;
                    } else if best_reading.is_none() {
                        best_reading = Some(field_name.clone());
                    }
                } else {
                    best_reading = Some(field_name.clone());
                    break;
                }
            }
        }
    }

    (best_term, best_reading)
}

fn is_likely_reading(value: &str) -> bool {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return false;
    }

    trimmed.chars().all(|c| is_char_kana(c) || c.is_whitespace())
}

fn is_likely_term(value: &str) -> bool {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return false;
    }

    trimmed.chars().all(|c| c.is_japanese() || c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::guess_sentence_field;

    fn fields(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_sentence_wins_in_realistic_notetypes() {
        // The two maintainer-provided note types (issue #3 feedback).
        let lapis = fields(&[
            "Word",
            "Word Reading",
            "Word Meaning",
            "Word Furigana",
            "Word Audio",
            "Sentence",
            "Sentence Meaning",
            "Sentence Furigana",
            "Sentence Audio",
            "Notes",
            "Pitch Accent",
            "Pitch Accent Notes",
            "Frequency",
            "Picture",
        ]);
        assert_eq!(guess_sentence_field(&HashMap::new(), &lapis), Some("Sentence".into()));

        let kiku = fields(&[
            "Expression",
            "ExpressionFurigana",
            "ExpressionReading",
            "ExpressionAudio",
            "SelectionText",
            "MainDefinition",
            "DefinitionPicture",
            "Sentence",
            "SentenceFurigana",
            "SentenceAudio",
            "Picture",
            "Glossary",
            "Hint",
            "IsWordAndSentenceCard",
            "IsClickCard",
            "IsSentenceCard",
            "IsAudioCard",
            "PitchPosition",
            "PitchCategories",
            "Frequency",
            "FreqSort",
            "MiscInfo",
        ]);
        assert_eq!(guess_sentence_field(&HashMap::new(), &kiku), Some("Sentence".into()));
    }

    #[test]
    fn derived_sentence_variants_are_skipped() {
        let f = fields(&["Expression", "SentenceFurigana", "SentenceAudio", "ExampleSentence"]);
        assert_eq!(guess_sentence_field(&HashMap::new(), &f), Some("ExampleSentence".into()));
        let none = fields(&["Expression", "SentenceFurigana", "SentenceAudio", "IsSentenceCard"]);
        assert_eq!(guess_sentence_field(&HashMap::new(), &none), None);
    }

    #[test]
    fn content_fallback_finds_japanese_sentences() {
        let f = fields(&["Front", "Back"]);
        let sample = HashMap::from([
            ("Front".to_string(), "食べる".to_string()),
            ("Back".to_string(), "毎日パンを<b>食べる</b>。".to_string()),
        ]);
        assert_eq!(guess_sentence_field(&sample, &f), Some("Back".into()));
        let short = HashMap::from([("Back".to_string(), "パン".to_string())]);
        assert_eq!(guess_sentence_field(&short, &f), None);
    }
}
