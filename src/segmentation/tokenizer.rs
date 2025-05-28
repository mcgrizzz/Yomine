use std::collections::HashMap;

use vibrato::{
    tokenizer::worker::Worker,
    Tokenizer,
};
use wana_kana::IsJapaneseStr;

use super::{
    rule_matcher::parse_into_words,
    token_models::{
        RawToken,
        UnidicToken,
        VibratoToken,
    },
    word::{
        Word,
        POS,
    },
};
use crate::{
    core::{
        utils::{
            pairwise_deinflection,
            NormalizeLongVowel,
        },
        Sentence,
        Term,
        YomineError,
    },
    dictionary::{
        frequency_manager::FrequencyManager,
        token_dictionary::{
            ensure_dictionary,
            load_dictionary,
            DictType,
        },
    },
};

pub fn extract_words(
    mut worker: Worker<'_>,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
) -> Vec<Term> {
    let mut terms = Vec::<Term>::new();

    for sentence in sentences.iter_mut() {
        worker.reset_sentence(&sentence.text);
        worker.tokenize();

        let tokens: Vec<UnidicToken> = worker
            .token_iter()
            .map(|token| {
                let vibrato_token: VibratoToken = VibratoToken {
                    surface: token.surface().to_string(),
                    features: token.feature().to_string(),
                };

                let surface = vibrato_token.surface.clone();
                let raw_token: RawToken = vibrato_token.into();

                (surface, raw_token).into()
            })
            .collect();

        let words: Vec<Word> = match parse_into_words(tokens) {
            Ok(parsed_words) => parsed_words,
            Err(_) => Vec::new(),
        };

        let mut sentence_terms: Vec<Term> = words
            .into_iter()
            .map(|word| {
                let mut term: Term = word.into();
                if term.surface_form.as_str().is_japanese() {
                    match term.part_of_speech {
                        POS::Verb
                        | POS::SuruVerb
                        | POS::AdjectivalNoun
                        | POS::Adjective
                        | POS::Noun => {
                            let deinflections: Vec<(String, String)> =
                                pairwise_deinflection(&term.surface_form, &term.surface_reading);

                            let mut sorted_deinflections: Vec<(String, String)> = deinflections
                                .into_iter()
                                .filter(|(word, reading)| {
                                    frequency_manager
                                        .get_harmonic_frequency_for_pair(word, reading)
                                        .is_some()
                                })
                                .collect();

                            sorted_deinflections.sort_by_key(|(word, reading)| {
                                frequency_manager.get_harmonic_frequency_for_pair(word, reading)
                            });

                            if sorted_deinflections.len() > 0 {
                                term.lemma_form = sorted_deinflections[0].0.clone();
                                term.lemma_reading = sorted_deinflections[0].1.clone();
                            }
                        }
                        _ => {}
                    }
                }

                let freq_map: HashMap<String, u32> = frequency_manager.build_freq_map(
                    &term.lemma_form,
                    &term.lemma_reading,
                    term.is_kana,
                );
                term.frequencies = freq_map;

                let index_in_sentence = sentence
                    .text
                    .match_indices(&term.surface_form)
                    .next()
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);

                term.sentence_references.push((sentence.id, index_in_sentence));

                term
            })
            .collect();

        sentence.segments.extend(sentence_terms.iter().map(|term| {
            let start_index = sentence
                .text
                .match_indices(&term.full_segment)
                .next()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let end_index = start_index + term.full_segment.len();
            (term.surface_reading.clone(), term.part_of_speech.clone(), start_index, end_index)
        }));

        //Add phrases without filtering the terms for now
        'outer: for start in 0..sentence_terms.len() {
            for end in (start + 1..sentence_terms.len()).rev() {
                let subrange = &sentence_terms[start..=end];
                let mut phrase: Term = Term::from_slice(subrange);

                let freq = frequency_manager.get_harmonic_frequency_for_pair(
                    &phrase.surface_form.normalize_long_vowel(),
                    &phrase.surface_reading.normalize_long_vowel(),
                );

                if let Some(frequency) = freq {
                    let word_frequencies: Vec<(String, f32)> = subrange
                        .iter()
                        .map(|term| {
                            let freq = term
                                .frequencies
                                .get("HARMONIC")
                                .cloned()
                                .unwrap_or(u32::max_value());
                            (term.lemma_form.to_string(), freq as f32)
                        })
                        .collect();

                    let mult_frequencies: f32 =
                        word_frequencies.iter().map(|(_, freq)| *freq as f32).product();

                    let k = subrange.len() as u32;
                    let score: f32 = (frequency as f32).powf(k as f32) / mult_frequencies;

                    let ratios: Vec<f32> = word_frequencies
                        .iter()
                        .map(|(_, freq)| (frequency as f32) / *freq)
                        .collect();

                    let max_ratio: f32 = ratios.iter().fold(0.0, |acc, &x| acc.max(x));
                    let char_count = phrase.lemma_form.chars().count();

                    let score_threshold = 10.0;
                    let ratio_threshold = 120.0;
                    let min_len = 4;

                    let override_ratio_threshold = 40.0;
                    let phrase_freq_threshold = 10000;

                    if char_count < min_len {
                        continue;
                    }

                    if frequency > phrase_freq_threshold && max_ratio < override_ratio_threshold {
                        continue;
                    }

                    phrase.part_of_speech = POS::NounExpression;
                    for term in subrange {
                        if !matches!(
                            term.part_of_speech,
                            POS::Noun | POS::CompoundNoun | POS::ProperNoun
                        ) {
                            phrase.part_of_speech = POS::Expression;
                            break;
                        }
                    }

                    if score <= score_threshold || max_ratio >= ratio_threshold {
                        phrase.frequencies.insert("HARMONIC".to_string(), frequency);
                        phrase
                            .sentence_references
                            .append(&mut sentence_terms[start].sentence_references.clone());

                        // println!(
                        //     "'{}': [{}, {}, {}, {}],",
                        //     &phrase.lemma_form,
                        //     frequency,
                        //     score,
                        //     max_ratio,
                        //     char_count,
                        // );

                        sentence_terms.push(phrase);

                        break 'outer; // Stop on the largest phrase that satisfies the heuristic
                    }
                }
            }
        }

        terms.append(&mut sentence_terms);
    }

    terms
}

pub fn init_vibrato(dict_type: &DictType) -> Result<Tokenizer, YomineError> {
    let dict_path = ensure_dictionary(dict_type)?;
    let dict = load_dictionary(dict_path.to_str().unwrap())?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}
