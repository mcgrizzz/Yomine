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
