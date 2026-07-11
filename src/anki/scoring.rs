use std::sync::Arc;

use wana_kana::IsJapaneseStr;

use super::types::Vocab;
use crate::{
    core::utils::{
        is_kanji_char,
        normalize_japanese_text,
    },
    dictionary::frequency_manager::FrequencyManager,
};

pub const KEEP_TERM_THRESHOLD: f32 = 0.60; //Pretty much anything over low-confidence.
pub const HIGH_CONFIDENCE_SCORE: f32 = 0.85; // Exact matches, kanji/kana pairs
pub const MEDIUM_CONFIDENCE_SCORE: f32 = 0.70; // Normalized variations
pub const LOW_CONFIDENCE_SCORE: f32 = 0.55; // Same readings only

const UNGRADED_PAIR_CONFIDENCE: f32 = 0.9; // Kanji/kana pair, no data to grade the reading

pub struct AnkiMatcher {
    frequency_manager: Arc<FrequencyManager>,
}

impl AnkiMatcher {
    pub fn new(frequency_manager: Arc<FrequencyManager>) -> Self {
        Self { frequency_manager }
    }

    /// Check if a part of speech indicates a content word (vs grammatical/functional word)
    pub fn is_content_word(&self, pos: &crate::segmentation::word::POS) -> bool {
        use crate::segmentation::word::POS;
        matches!(
            pos,
            POS::Noun
                | POS::ProperNoun
                | POS::CompoundNoun
                | POS::Verb
                | POS::SuruVerb
                | POS::Adjective
                | POS::AdjectivalNoun
                | POS::Adverb
        )
    }

    pub fn inclusivity_score(
        &self,
        yomine_word: &str,
        yomine_reading: &str,
        anki_card: &Vocab,
        pos: &crate::segmentation::word::POS,
    ) -> f32 {
        let anki_word = anki_card.term.as_str();
        let anki_reading = anki_card.reading.as_str();

        // Normalize all text upfront for consistent comparison
        let norm_yomine_word = normalize_japanese_text(yomine_word);
        let norm_anki_word = normalize_japanese_text(anki_word);
        let norm_yomine_reading = normalize_japanese_text(yomine_reading);
        let norm_anki_reading = normalize_japanese_text(anki_reading);

        // 1. Perfect match (exact word and reading) + hiragana to katakana matching since it's often an artistic choice
        if (yomine_word == anki_word || yomine_word.is_kana() && anki_word.is_kana())
            && norm_yomine_reading == norm_anki_reading
        {
            return 1.0;
        }

        // 2. We do not do partial matching on non-content words like particles etc. They're too short to trust those partial matches.
        if !self.is_content_word(pos) {
            return 0.0;
        }

        //3. High confidence: exact word match
        if yomine_word == anki_word {
            return HIGH_CONFIDENCE_SCORE;
        }

        //OR kanji/kana pair with same reading, for example: 事 (こと) against こと (こと)
        // Check kanji/kana pair with frequency-based confidence
        if norm_yomine_reading == norm_anki_reading {
            let kanji_kana_confidence =
                self.is_kanji_kana_pair(yomine_word, anki_word, &norm_yomine_reading);
            if kanji_kana_confidence > 0.0 {
                // Use the frequency-based confidence to modulate the score
                return HIGH_CONFIDENCE_SCORE * kanji_kana_confidence;
            }
        }

        // 4. Medium confidence: normalized forms match
        if norm_yomine_word == norm_anki_word && norm_yomine_reading == norm_anki_reading {
            return MEDIUM_CONFIDENCE_SCORE;
        }

        // 4b. High confidence: okurigana spelling variants with the same reading
        if norm_yomine_reading == norm_anki_reading
            && is_okurigana_variant_pair(yomine_word, anki_word)
        {
            return HIGH_CONFIDENCE_SCORE;
        }

        // 5. Low confidence: same reading, different kanji (both non-kana), this is below the threshold but maybe in the future we can use this.
        //Sometimes the same words have different kanji but are still the same word carrying different nuance.
        if !yomine_word.is_kana()
            && !anki_word.is_kana()
            && norm_yomine_reading == norm_anki_reading
        {
            return LOW_CONFIDENCE_SCORE;
        }

        0.0
    }

    /// Check if words form a validated kanji/kana pair using frequency data
    fn is_kanji_kana_pair(&self, word1: &str, word2: &str, reading: &str) -> f32 {
        use std::collections::HashMap;

        // Basic structural check first
        let is_structural_pair =
            (word1.is_kana() && !word2.is_kana()) || (!word1.is_kana() && word2.is_kana());
        if !is_structural_pair {
            return 0.0;
        }

        // Determine which is kanji and which is kana
        let kanji_word = if word1.is_kana() { word2 } else { word1 };

        // Get all frequency data for the kanji word
        let frequencies = self.frequency_manager.get_frequency_data_by_term(kanji_word);
        if frequencies.is_empty() {
            return UNGRADED_PAIR_CONFIDENCE;
        }

        // Group frequencies by reading and calculate averages
        let mut grouped_frequencies: HashMap<String, Vec<f32>> = HashMap::new();
        for freq in frequencies {
            if let Some(freq_reading) = freq.reading() {
                grouped_frequencies
                    .entry(freq_reading.to_string())
                    .or_insert_with(Vec::new)
                    .push(freq.value() as f32);
            }
        }

        if grouped_frequencies.is_empty() {
            return UNGRADED_PAIR_CONFIDENCE;
        }

        let average_frequencies: Vec<(String, f32)> = grouped_frequencies
            .into_iter()
            .map(|(reading, values)| {
                let avg_freq = values.iter().sum::<f32>() / (values.len() as f32);
                (reading, avg_freq)
            })
            .collect();

        let (min_freq, max_freq) = average_frequencies
            .iter()
            .fold((f32::MAX, f32::MIN), |(min, max), (_, freq)| (min.min(*freq), max.max(*freq)));

        //The farther the reading gets from the most likely reading for the kanji, the less likely this is a match
        if let Some((_, matched_freq)) =
            average_frequencies.iter().find(|(freq_reading, _)| freq_reading == reading)
        {
            if max_freq > min_freq {
                let normalized = (*matched_freq - min_freq) / (max_freq - min_freq);
                let probability = 1.0 - (0.1 + normalized * 0.8);
                probability
            } else {
                0.9
            }
        } else {
            0.0
        }
    }
}

/// The forms differ only where one side writes a segment in kanji and the
/// other in kana (話し掛ける vs 話しかける ✓; 上る vs 昇る ✗).
fn is_okurigana_variant_pair(a: &str, b: &str) -> bool {
    if a.is_kana() || b.is_kana() {
        return false;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let prefix = a_chars.iter().zip(&b_chars).take_while(|(x, y)| x == y).count();
    let max_suffix = a_chars.len().min(b_chars.len()) - prefix;
    let suffix = a_chars
        .iter()
        .rev()
        .zip(b_chars.iter().rev())
        .take(max_suffix)
        .take_while(|(x, y)| x == y)
        .count();
    let mid_a: String = a_chars[prefix..a_chars.len() - suffix].iter().collect();
    let mid_b: String = b_chars[prefix..b_chars.len() - suffix].iter().collect();

    let kanji_vs_kana = |kanji: &str, kana: &str| {
        kanji.chars().any(is_kanji_char) && !kana.is_empty() && kana.is_kana()
    };
    kanji_vs_kana(&mid_a, &mid_b) || kanji_vs_kana(&mid_b, &mid_a)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        dictionary::{
            frequency_dict::FrequencyDictionary,
            JsonFrequency,
            JsonFrequencyData,
            TermMetaBankV3,
        },
        segmentation::word::POS,
    };

    fn matcher_with(entries: &[(&str, &str, u32)]) -> AnkiMatcher {
        let metas = entries
            .iter()
            .map(|(term, reading, rank)| TermMetaBankV3 {
                term: term.to_string(),
                data_type: "freq".to_string(),
                data: Some(JsonFrequencyData::Nested {
                    reading: reading.to_string(),
                    frequency: JsonFrequency::Number(*rank),
                }),
            })
            .collect();
        let dicts = if entries.is_empty() {
            Vec::new()
        } else {
            vec![FrequencyDictionary::new("TEST".to_string(), "test".to_string(), metas)]
        };
        AnkiMatcher::new(Arc::new(FrequencyManager::from_dictionaries(dicts)))
    }

    fn vocab(term: &str, reading: &str) -> Vocab {
        Vocab {
            term: term.to_string(),
            reading: reading.to_string(),
            card_id: None,
            interval: None,
        }
    }

    #[test]
    fn kana_subtitle_matches_kanji_card_without_frequency_data() {
        let m = matcher_with(&[]);
        let score = m.inclusivity_score("だます", "だます", &vocab("騙す", "だます"), &POS::Verb);
        assert!(score >= KEEP_TERM_THRESHOLD, "got {score}");
    }

    #[test]
    fn frequency_graded_kanji_kana_pair_matches() {
        let m = matcher_with(&[("騙す", "だます", 3156)]);
        let score = m.inclusivity_score("だます", "だます", &vocab("騙す", "だます"), &POS::Verb);
        assert!(score >= KEEP_TERM_THRESHOLD, "got {score}");
    }

    #[test]
    fn mixed_script_card_matches_kana_form() {
        let m = matcher_with(&[]);
        let score =
            m.inclusivity_score("どういう", "どういう", &vocab("どう言う", "どういう"), &POS::Verb);
        assert!(score >= KEEP_TERM_THRESHOLD, "got {score}");
        let score = m.inclusivity_score(
            "いくらでも",
            "いくらでも",
            &vocab("幾らでも", "いくらでも"),
            &POS::Adverb,
        );
        assert!(score >= KEEP_TERM_THRESHOLD, "got {score}");
    }

    #[test]
    fn okurigana_variants_match() {
        let m = matcher_with(&[]);
        let score = m.inclusivity_score(
            "話し掛ける",
            "はなしかける",
            &vocab("話しかける", "はなしかける"),
            &POS::Verb,
        );
        assert!(score >= KEEP_TERM_THRESHOLD, "got {score}");
    }

    #[test]
    fn different_kanji_with_same_reading_stay_apart() {
        let m = matcher_with(&[]);
        for (a, b, reading) in [("上る", "昇る", "のぼる"), ("橋", "箸", "はし")] {
            let score = m.inclusivity_score(a, reading, &vocab(b, reading), &POS::Noun);
            assert!(score < KEEP_TERM_THRESHOLD, "{a} vs {b} got {score}");
        }
    }

    #[test]
    fn contradicted_kanji_kana_pair_stays_apart() {
        // 犬's only dictionary reading is いぬ, so a けん kana form is not it.
        let m = matcher_with(&[("犬", "いぬ", 300)]);
        let score = m.inclusivity_score("けん", "けん", &vocab("犬", "けん"), &POS::Noun);
        assert!(score < KEEP_TERM_THRESHOLD, "got {score}");
    }
}
