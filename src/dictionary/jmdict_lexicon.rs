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
    fn readings_match_only_within_one_entry() {
        assert!(same_entry("明日", "あす", "あした"));
        assert!(same_entry("日本", "ニッポン", "にほん"));
        assert!(!same_entry("表", "ひょう", "おもて"));
        assert!(!same_entry("方", "ほう", "かた"));
    }
}
