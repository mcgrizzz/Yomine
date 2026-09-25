//! Grammar phrases, which auto mode treats with care: a card can land on the wrong sense.
use wana_kana::IsJapaneseStr;

use crate::{
    core::models::Term,
    dictionary::jmdict_lexicon,
    segmentation::{
        unidic_tags::UnidicTag,
        word::{
            Word,
            POS,
        },
    },
};

/// Nouns that only carry grammar (ことになる, わけがない).
const FORMAL_NOUNS: &[&str] = &["事", "物", "所", "訳", "筈"];

/// Particles, copulas, bound words (ない, なる, ある) and formal nouns.
pub(super) fn is_grammatical(word: &Word) -> bool {
    matches!(word.part_of_speech, POS::Postposition | POS::Copula)
        || word.tokens.first().is_some_and(|t| {
            matches!(t.pos2, UnidicTag::Hijiritsukanou | UnidicTag::Jodoushigokan)
                || (t.surface.as_str().is_kana() && FORMAL_NOUNS.contains(&t.lexeme.as_str()))
        })
}

/// A phrase of grammatical words only that JMdict gives several senses (ことになる, じゃない).
pub(super) fn is_ambiguous(grammatical: &[bool], phrase: &Term) -> bool {
    grammatical.iter().all(|&g| g)
        && [&phrase.surface_form, &phrase.lemma_form]
            .iter()
            .any(|form| jmdict_lexicon::phrase_senses(form).is_some_and(|senses| senses > 1))
}
