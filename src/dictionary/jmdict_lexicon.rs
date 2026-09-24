//! Generated JMdict phrase forms and per-entry readings (research/lexical/compile_lexicon.py).
use super::{
    frequency_manager::fold_katakana,
    kana_preference::SortedIndex,
};
use crate::core::utils::NormalizeLongVowel;

const DATA: &[u8] = include_bytes!("../../assets/jmdict-lexicon.bin");

fn bundled() -> SortedIndex<'static> {
    SortedIndex::from_bytes(DATA, b"LEXIDX01").expect("validated bundled lexicon")
}

fn lookup(key: String) -> Option<&'static [u8]> {
    bundled().lookup(key.as_bytes())
}

/// JMdict lists `form` as an expression, adverb, conjunction or particle (ついでに, に取って).
pub(crate) fn is_phrase(form: &str) -> bool {
    lookup(format!("p\t{}", form.normalize_long_vowel())).is_some()
}

/// One JMdict entry lists `spelling` under both readings (明日 as あした and あす).
pub(crate) fn same_entry(spelling: &str, reading: &str, other: &str) -> bool {
    let entries = |reading: &str| {
        lookup(format!("e\t{spelling}\t{}", fold_katakana(reading).normalize_long_vowel()))
    };
    let (Some(a), Some(b)) = (entries(reading), entries(other)) else {
        return false;
    };
    a.chunks_exact(4).any(|id| b.chunks_exact(4).any(|other| other == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_are_jmdict_expressions_not_particle_ngrams() {
        for form in ["ついでに", "にとって", "に取って", "について", "しょうがない", "ところが"]
        {
            assert!(is_phrase(form), "{form}");
        }
        for form in ["あなたに", "いたから", "したんだ", "子らしい", "にカット"]
        {
            assert!(!is_phrase(form), "{form}");
        }
    }

    #[test]
    fn readings_match_only_within_one_entry() {
        assert!(same_entry("明日", "あす", "あした"));
        assert!(same_entry("日本", "ニッポン", "にほん"));
        assert!(!same_entry("表", "ひょう", "おもて"));
        assert!(!same_entry("方", "ほう", "かた"));
    }
}
