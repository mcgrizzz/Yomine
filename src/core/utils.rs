use std::cell::OnceCell;

use jp_deinflector::deinflect;
use regex::Regex;

pub trait NormalizeLongVowel {
    fn normalize_long_vowel(&self) -> String;
}

//とおい -> とうい AND  けいたい -> けいたい
impl NormalizeLongVowel for str {
    fn normalize_long_vowel(&self) -> String {
        // OnceCell will only compile the Regex once
        let cell = OnceCell::new();
        let re: &Regex = cell.get_or_init(|| {
            Regex::new(
                r"([おこそとのほもよろごぞどぼぽ])お|([けせてねへめれげぜでべぺ])え"
            ).unwrap()
        });

        re.replace_all(self, |captures: &regex::Captures| {
            if let Some(o_row) = captures.get(1) {
                format!("{}う", o_row.as_str())
            } else if let Some(e_row) = captures.get(2) {
                format!("{}い", e_row.as_str())
            } else {
                captures[0].to_string() // Fallback (should never be reached)
            }
        }).to_string()
    }
}

/// Implement the trait for `String` by forwarding the method to `str`
impl NormalizeLongVowel for String {
    fn normalize_long_vowel(&self) -> String {
        self.as_str().normalize_long_vowel()
    }
}

pub fn harmonic_frequency(nums: &Vec<u32>) -> Option<u32> {
    let mut sum_of_reciprocals = 0.0;
    let mut count = 0;

    nums.iter().for_each(|num| {
        if num > &0 {
            sum_of_reciprocals += 1.0 / (*num as f32);
            count += 1;
        }
    });

    if count > 0 {
        Some(((count as f32) / sum_of_reciprocals).round() as u32)
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
                ("行っる".to_string(), "いっる".to_string())
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
            ("読ます".to_string(), "よます".to_string())
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

use serde::{ Deserialize, Deserializer };
use wana_kana::utils::is_char_kana;


fn kana_suffix_length(word: &str) -> usize {
    word.chars()
        .rev()
        .take_while(|&c| is_char_kana(c))
        .count()
}

fn kanji_mapping(word: &str, reading: &str) -> (String,  String) {
    let suffix_len = kana_suffix_length(word);
    let base_len =  word.chars().count() - suffix_len;

    let base: String = word.chars().take(base_len).collect();
    let base_reading: String = reading.chars().take(reading.chars().count() - suffix_len).collect();

    (base, base_reading)
}

pub fn pairwise_deinflection(word: &str, reading: &str) -> Vec<(String, String)> {
    let mut results = vec![(word.to_string(), reading.to_string())];
    let deinflections = deinflect(word);

    // Early return if no further deinflection is needed
    if deinflections.len() <= 1 {
        return results;
    }

    let (base, base_reading) = kanji_mapping(word, reading);

    for deinflected_word in deinflections {
        let adjusted_reading = deinflected_word.replacen(&base, &base_reading, 1);
        results.push((deinflected_word, adjusted_reading));
    }

    results
}

pub fn deserialize_number_or_numeric_string<'de, D>(deserializer: D) -> Result<u32, D::Error>
    where D: Deserializer<'de>
{
    // Use serde_json::Value as an intermediate type to handle both numbers and strings
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::Number(num) => {
            if let Some(n) = num.as_u64() {
                if n <= (u32::MAX as u64) {
                    Ok(n as u32)
                } else {
                    Err(serde::de::Error::custom(format!("number {} is too large for u32", n)))
                }
            } else {
                Err(serde::de::Error::custom("number cannot be converted to u32"))
            }
        }
        // Handle JSON strings (e.g., "123")
        serde_json::Value::String(s) =>
            match s.parse::<u32>() {
                Ok(num) => Ok(num),
                Err(_) =>
                    Err(serde::de::Error::custom(format!("string '{}' is not a valid number", s))),
            }
        // Reject anything else (e.g., objects, arrays, booleans)
        _ =>
            Err(
                serde::de::Error::custom(
                    format!("expected a number or numeric string, got: {}", value)
                )
            ),
    }
}
