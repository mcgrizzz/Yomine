//! Generated JMdict phrase forms, spellings with their readings, and per-entry readings (research/lexical/compile_lexicon.py).
use super::{
    frequency_manager::fold_katakana,
    kana_preference::SortedIndex,
};
use crate::{
    core::utils::NormalizeLongVowel,
    segmentation::word::POS,
};

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

/// JMdict lists `form` as a word spelled with kanji, of any part of speech (二日酔い), or as a
/// word usually written in kana (もったいない), whose readings the lexicon keeps in hiragana.
pub(crate) fn is_word(form: &str) -> bool {
    let form = &ascii_width(form);
    let word = |form: &str| lookup(format!("w\t{}", form.normalize_long_vowel())).is_some();
    word(form) || word(&fold_katakana(form))
}

/// The readings JMdict gives a spelling with kanji, in JMdict's order (明日: あした, あす).
pub(crate) fn readings(form: &str) -> Vec<&'static str> {
    let form = ascii_width(form);
    lookup(format!("w\t{}", form.normalize_long_vowel()))
        .into_iter()
        .flat_map(|value| value.split(|&byte| byte == b'\t').skip(1))
        .filter_map(|reading| std::str::from_utf8(reading).ok())
        .collect()
}

/// The type JMdict's first sense gives `form` (本当に an adverb, お迎え a noun), when the
/// lexicon lists it as a word or phrase.
pub(crate) fn word_type(form: &str) -> Option<POS> {
    let form = ascii_width(form);
    let form = form.normalize_long_vowel();
    let code = lookup(format!("w\t{form}"))
        .and_then(|value| value.first())
        .or_else(|| lookup(format!("p\t{form}")).and_then(|value| value.get(1)))?;
    Some(match code {
        b'v' => POS::Verb,
        b'i' => POS::Adjective,
        b'a' => POS::AdjectivalNoun,
        b'n' => POS::Noun,
        b'o' => POS::Pronoun,
        b'd' => POS::Adverb,
        b'c' => POS::Conjunction,
        b'j' => POS::Interjection,
        b'p' => POS::Determiner,
        b'e' => POS::Expression,
        _ => return None,
    })
}

/// The lexicon keys spellings in NFKC, which writes full-width letters and digits as ASCII
/// (１度 as 1度).
fn ascii_width(form: &str) -> String {
    form.chars()
        .map(|c| match c {
            '！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// JMdict lists `reading` as an i-adjective ending in ない (つまらない, くだらない).
pub(crate) fn is_nai_adjective(reading: &str) -> bool {
    lookup(format!("a\t{}", reading.normalize_long_vowel())).is_some()
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
    fn usually_kana_content_words_are_words() {
        for form in ["もったいない", "ありえない", "カッコよい"] {
            assert!(is_word(form), "{form}");
        }
        for form in ["こういう", "いいか"] {
            assert!(!is_word(form), "{form}");
        }
    }

    #[test]
    fn two_character_spellings_are_words() {
        for form in ["一体", "お金", "１度"] {
            assert!(is_word(form), "{form}");
        }
    }

    #[test]
    fn listed_forms_carry_jmdict_types() {
        for (form, pos) in [
            ("本当に", POS::Adverb),
            ("それとも", POS::Conjunction),
            ("お迎え", POS::Noun),
            ("大丈夫", POS::AdjectivalNoun),
            ("ことになる", POS::Expression),
        ] {
            assert_eq!(word_type(form), Some(pos), "{form}");
        }
        assert_eq!(word_type("あなたに"), None);
    }

    #[test]
    fn spellings_carry_their_readings() {
        assert_eq!(readings("甘納豆"), ["あまなっとう"]);
        assert_eq!(readings("通り")[0], "とおり");
        assert!(readings("明日").starts_with(&["あした", "あす"]));
        assert!(readings("もったいない").is_empty());
    }

    #[test]
    fn readings_match_only_within_one_entry() {
        assert!(same_entry("明日", "あす", "あした"));
        assert!(same_entry("日本", "ニッポン", "にほん"));
        assert!(!same_entry("表", "ひょう", "おもて"));
        assert!(!same_entry("方", "ほう", "かた"));
    }
}
