use std::sync::Arc;

use wana_kana::{
    ConvertJapanese,
    IsJapaneseStr,
};

use super::types::Vocab;
use crate::{
    core::utils::NormalizeLongVowel,
    dictionary::frequency_manager::FrequencyManager,
};

pub const KEEP_TERM_THRESHOLD: f32 = 0.60; //Pretty much anything over low-confidence.
pub const HIGH_CONFIDENCE_SCORE: f32 = 0.85; // Exact matches, kanji/kana pairs
pub const MEDIUM_CONFIDENCE_SCORE: f32 = 0.70; // Normalized variations
pub const LOW_CONFIDENCE_SCORE: f32 = 0.55; // Same readings only

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
        let norm_yomine_word = self.normalize_japanese_text(yomine_word);
        let norm_anki_word = self.normalize_japanese_text(anki_word);
        let norm_yomine_reading = yomine_reading.to_hiragana();
        let norm_anki_reading = anki_reading.to_hiragana();

        // 1. Perfect match (exact word and reading)
        if yomine_word == anki_word && norm_yomine_reading == norm_anki_reading {
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
            return 0.0;
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
            return 0.0;
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

    /// Normalize Japanese text for comparison
    fn normalize_japanese_text(&self, text: &str) -> String {
        // Only convert to hiragana for consistent kana comparison, preserve actual characters
        text.to_hiragana().normalize_long_vowel().to_string()
    }
}
