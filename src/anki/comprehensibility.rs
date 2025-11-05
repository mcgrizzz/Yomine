//Until we can get FSRS, we're using interval

use crate::core::{
    Sentence,
    Term,
};

pub fn comp_term(interval: Option<f32>, known_interval: u32) -> f32 {
    match interval {
        None => 0.0,
        Some(days) => {
            let ratio = (days + 1.0) / (known_interval as f32 + 1.0);
            if ratio >= 1.0 {
                1.0
            } else {
                // logarthimic scaling... We can do any sort of scaling here. Will have to test to see what feels closest to people's experiences,
                // not necessarily worth spending a lot of time yet since we will eventually be able to grab FSRS info
                (days + 1.0).ln() / (known_interval as f32 + 1.0).ln()
            }
        }
    }
}

/// Calculate sentence comprehension as average of its terms, again there's a bunch of ways to weigh terms. Hatsuon length, character number, etc.
pub fn calculate_sentence_comprehension(sentence: &mut Sentence, terms: &[Term]) {
    let sentence_terms: Vec<&Term> = terms
        .iter()
        .filter(|t| t.sentence_references.iter().any(|(sid, _)| *sid == sentence.id))
        .collect();

    if sentence_terms.is_empty() {
        sentence.comprehension = 0.0;
        return;
    }

    // Average comprehension of all terms in the sentence
    let sum: f32 = sentence_terms.iter().map(|t| t.comprehension).sum();
    sentence.comprehension = sum / sentence_terms.len() as f32;
}
