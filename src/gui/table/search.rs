use crate::core::{
    utils::text_matches_search,
    Sentence,
    Term,
};

pub fn matches_search(term: &Term, sentences: &[Sentence], query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    // Check term forms and readings using unified search logic
    if text_matches_search(&term.lemma_form, query)
        || text_matches_search(&term.surface_form, query)
        || text_matches_search(&term.full_segment, query)
        || text_matches_search(&term.lemma_reading, query)
        || text_matches_search(&term.surface_reading, query)
        || text_matches_search(&term.part_of_speech.to_string(), query)
    {
        return true;
    }

    // Search in sentence text
    for (sentence_idx, _) in &term.sentence_references {
        if let Some(sentence) = sentences.get(*sentence_idx) {
            if text_matches_search(&sentence.text, query) {
                return true;
            }
        }
    }

    false
}
