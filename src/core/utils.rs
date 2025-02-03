use std::{cell::OnceCell, collections::HashMap};

use regex::Regex;
use wana_kana::{IsJapaneseChar, IsJapaneseStr};

use super::Term;

pub trait SwapLongVowel {
    fn swap_long_vowel(&self) -> String;
}

//とおい -> とうい AND  けいたい -> けいたい
impl SwapLongVowel for str {
    fn swap_long_vowel(&self) -> String {
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
impl SwapLongVowel for String {
    fn swap_long_vowel(&self) -> String {
        self.as_str().swap_long_vowel()
    }
}

impl Term {
    pub fn get_surface_reading(&self) -> String {

        if self.surface_form.as_str().is_kana() {
            return self.surface_form.clone();
        }

        if self.surface_form == self.lemma_form {
            return self.lemma_reading.clone();
        }
        
        let mut reading = self.lemma_reading.clone();

        // If it's a verb, remove the last u-row kana
        if self.part_of_speech.is_verb() {
            if let Some(last_char) = reading.chars().last() {
                if "うくすつぬふぶむる".contains(last_char) {
                    reading.pop();
                }
            }
        //Adjectives, should be the same basic logic. Grab everything before い
        } else if self.part_of_speech.is_i_adjective() {
            if let Some(last_char) = reading.chars().last() {
                if 'い' == last_char {
                    reading.pop();
                }
            }
        }

        
        let mut buffer = String::new();
        let mut reading_chars = reading.chars();
        
        for surface_char in self.surface_form.chars() {
            if self.lemma_form.contains(surface_char) {
                if let Some(reading_char) = reading_chars.next() {
                    buffer.push(reading_char);
                }
            } else {
                buffer.push(surface_char);
            }
        }

        buffer
    }
}

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