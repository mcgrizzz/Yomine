use std::{cell::OnceCell, collections::HashMap};

use jp_deinflector::deinflect;
use regex::Regex;
use serde::de;
use wana_kana::{IsJapaneseChar, IsJapaneseStr};

use super::Term;

pub trait NormalizeLongVowel {
    fn normalize_long_vowel(&self) -> String;
}

//とおい -> とうい AND  けいたい -> けいたい
impl NormalizeLongVowel for str {
    fn normalize_long_vowel(&self) -> String {
        // OnceCell will only compile the Regex once
        let cell = OnceCell::new();
        let re: &Regex = cell.get_or_init(|| {
            Regex::new(r"([おこそとのほもよろごぞどぼぽ])お|([けせてねへめれげぜでべぺ])え").unwrap()
        });

        re.replace_all(self, |captures: &regex::Captures| {
            if let Some(o_row) = captures.get(1) {
                format!("{}う", o_row.as_str())
            } else if let Some(e_row) = captures.get(2) {
                format!("{}い", e_row.as_str())
            } else {
                captures[0].to_string() // Fallback (should never be reached)
            }
        })
        .to_string()
    }
}

/// Implement the trait for `String` by forwarding the method to `str`
impl NormalizeLongVowel for String {
    fn normalize_long_vowel(&self) -> String {
        self.as_str().normalize_long_vowel()
    }
}

// impl Term {
//     pub fn get_surface_reading(&self) -> String {

//         if self.surface_form.as_str().is_kana() {
//             return self.surface_form.clone();
//         }

//         if self.surface_form == self.lemma_form {
//             return self.lemma_reading.clone();
//         }
        
//         let mut reading = self.lemma_reading.clone();

//         // If it's a verb, remove the last u-row kana
//         if self.part_of_speech.is_verb() {
//             if let Some(last_char) = reading.chars().last() {
//                 if "うくすつぬふぶむる".contains(last_char) {
//                     reading.pop();
//                 }
//             }
//         //Adjectives, should be the same basic logic. Grab everything before い
//         } else if self.part_of_speech.is_i_adjective() {
//             if let Some(last_char) = reading.chars().last() {
//                 if 'い' == last_char {
//                     reading.pop();
//                 }
//             }
//         }

        
//         let mut buffer = String::new();
//         let mut reading_chars = reading.chars();
        
//         for surface_char in self.surface_form.chars() {
//             if self.lemma_form.contains(surface_char) {
//                 if let Some(reading_char) = reading_chars.next() {
//                     buffer.push(reading_char);
//                 }
//             } else {
//                 buffer.push(surface_char);
//             }
//         }

//         buffer
//     }
// }

pub fn harmonic_frequency(nums: &Vec<u32>) -> Option<u32> {
    let mut sum_of_reciprocals = 0.0;
    let mut count = 0;

    nums.iter().for_each(|num| {
        if num > &0 {
            sum_of_reciprocals += 1.0 / *num as f32;
            count += 1;
        }
    });

    if count > 0 {
        Some((count as f32 / sum_of_reciprocals).round() as u32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairwise_deinflection_with_irregular_reading() {
        let word = "行った";
        let reading = "いった";
        let result = pairwise_deinflection(word, reading);
        assert_eq!(
            result,
            vec![
                ("行った".to_string(), "いった".to_string()),
                ("行く".to_string(), "いく".to_string()),
                ("行う".to_string(), "いう".to_string()), //We will filter out weird stuff like this later. Right now we just want our diffs to be consistent
                ("行つ".to_string(), "いつ".to_string()),
                ("行る".to_string(), "いる".to_string()),
                ("行っる".to_string(), "いっる".to_string()),
            ]
        );
    }


    #[test]
    fn test_pairwise_deinflection_causative_and_passive() {
        let word = "読ませられる";
        let reading = "よませられる";
        let result = pairwise_deinflection(word, reading);
        
        let expected = vec![
            ("読ませられる".to_string(), "よませられる".to_string()),
            ("読まする".to_string(), "よまする".to_string()),
            ("読ませる".to_string(), "よませる".to_string()),
            ("読ませらる".to_string(), "よませらる".to_string()),
            ("読む".to_string(), "よむ".to_string()),
            ("読ます".to_string(), "よます".to_string()),
        ];
        
        assert_eq!(result, expected);
    }

    #[test]
    fn test_pairwise_deinflection_omoimashita() {
        let word = "思いました";
        let reading = "おもいました";
        let result = pairwise_deinflection(word, reading);
        
        let expected = vec![
            ("思いました".to_string(), "おもいました".to_string()), 
            ("思う".to_string(), "おもう".to_string()), 
            ("思いる".to_string(), "おもいる".to_string()), 
            ("思います".to_string(), "おもいます".to_string()), 
            ("思いまする".to_string(), "おもいまする".to_string()), 
            ("思いましる".to_string(), "おもいましる".to_string())
        ];
        
        assert_eq!(result, expected);
    }
}



use difference::Changeset;
use wana_kana::utils::{is_char_kanji, is_char_kana};

// Applies deinflection rules step-by-step, adjusting the reading accordingly.
// Does not check word existence; filter later with dictionaries.

pub fn pairwise_deinflection(word: &str, reading: &str) -> Vec<(String, String)> {
    let mut results = vec![(word.to_string(), reading.to_string())];
    let deinflections = deinflect(word);

    // Early return if no further deinflection is needed
    if deinflections.len() <= 1 {
        return results;
    }

    // Compute stem and stem reading from the original word and reading
    let stem = initial_kanji_stem(word);
    let trailing_kana = trailing_kana_len(word);
    let stem_reading_len = reading.chars().count().saturating_sub(trailing_kana);
    let stem_reading = reading.chars().take(stem_reading_len).collect::<String>();

    let mut current_word = word.to_string();
    let mut current_reading = reading.to_string();

    for deinflected_word in deinflections {
        if deinflected_word == current_word {
            continue;
        }

        let adjusted_reading = if deinflected_word.starts_with(&stem) {
            // If the deinflected word preserves the stem, use stem_reading + ending
            let ending = &deinflected_word[stem.len()..];
            stem_reading.clone() + ending
        } else {
            // Fallback to original diff logic if stem changes (rare in this context)
            let diff = Changeset::new(&current_word, &deinflected_word, "");
            let mut adjusted = String::new();
            let mut reading_iter = current_reading.chars();

            for change in diff.diffs {
                match change {
                    difference::Difference::Same(text) => {
                        let same_len = text.chars().count();
                        adjusted.extend(reading_iter.by_ref().take(same_len));
                    }
                    difference::Difference::Rem(text) => {
                        let rem_len = text.chars().count();
                        reading_iter.by_ref().take(rem_len).for_each(drop);
                    }
                    difference::Difference::Add(text) => {
                        adjusted.push_str(&text);
                    }
                }
            }
            adjusted
        };

        results.push((deinflected_word.clone(), adjusted_reading.clone()));
        current_word = deinflected_word.clone();
        current_reading = adjusted_reading;
    }

    results
}

// Helper function to get the initial kanji stem
fn initial_kanji_stem(word: &str) -> String {
    word.chars().take_while(|&c| is_char_kanji(c)).collect()
}

// Helper function to count trailing kana characters
fn trailing_kana_len(word: &str) -> usize {
    word.chars().rev().take_while(|&c| is_char_kana(c)).count()
}
