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

/// Subsidiary verbs and the subsidiary adjective (補助動詞, 補助形容詞), which follow a て-form
/// to add aspect, direction, benefit or a wish (連れていく, 教えてくださる, 来てほしい), by UniDic
/// lexeme: おる and ゆく are 居る and 行く. UniDic's 非自立可能 tag is broader (終わる in 言って
/// 終わり). Contractions (ちゃう, とく, てる) are auxiliaries with no て of their own.
const SUBSIDIARIES: &[&str] = &[
    "居る",
    "有る",
    "置く",
    "仕舞う",
    "行く",
    "来る",
    "見る",
    "見せる",
    "上げる",
    "遣る",
    "呉れる",
    "貰う",
    "差し上げる",
    "下さる",
    "頂く",
    "いらっしゃる",
    "参る",
    "欲しい",
];

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

/// One of `SUBSIDIARIES`; whether it follows a て-form is the caller's to check.
pub(super) fn is_subsidiary(word: &Word) -> bool {
    word.tokens.first().is_some_and(|t| {
        matches!(t.pos1, UnidicTag::Doushi | UnidicTag::Keiyoushi)
            && SUBSIDIARIES.contains(&t.lexeme.as_str())
    })
}
