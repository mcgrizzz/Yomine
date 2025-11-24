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

    for (ord, sentence) in sentences.iter_mut().enumerate() {
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

                term.sentence_references.push((ord, index_in_sentence));

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
                        let index_in_sentence = sentence
                            .text
                            .match_indices(&phrase.surface_form)
                            .next()
                            .map(|(idx, _)| idx)
                            .unwrap_or(0);

                        phrase.sentence_references.push((ord, index_in_sentence));
                        let freq_map = frequency_manager.build_freq_map(
                            &phrase.lemma_form,
                            &phrase.lemma_reading,
                            phrase.is_kana,
                        );
                        phrase.frequencies = freq_map;
                        sentence_terms.push(phrase);

                        break 'outer; // Stop on the largest phrase that satisfies the heuristic
                    }
                }
            }
        }

        for term in terms.iter_mut() {
            if let Some(sentence_term) =
                sentence_terms.iter().find(|st| st.lemma_form == term.lemma_form)
            {
                for sentence_ref in &sentence_term.sentence_references {
                    if !term.sentence_references.contains(sentence_ref) {
                        term.sentence_references.push(*sentence_ref);
                    }
                }
            }
        }

        sentence_terms.retain(|sentence_term| {
            !terms.iter().any(|existing_term| {
                existing_term.lemma_form == sentence_term.lemma_form
                    && existing_term.lemma_reading == sentence_term.lemma_reading
            })
        });

        terms.append(&mut sentence_terms);
    }

    terms
}

pub fn extract_words_for_frequency(
    mut worker: Worker<'_>,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
    progress_callback: Option<&(dyn Fn(bool, usize, usize) + Sync)>,
) -> Vec<Term> {
    use rayon::prelude::*;

    let total_sentences = sentences.len();

    // Phase 1: Tokenize all sentences sequentially
    let mut sentence_tokens: Vec<(usize, String, Vec<UnidicToken>)> =
        Vec::with_capacity(total_sentences);
    let report_interval = std::cmp::min(1000, std::cmp::max(1, total_sentences / 20));

    for (ord, sentence) in sentences.iter().enumerate() {
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

        sentence_tokens.push((ord, sentence.text.clone(), tokens));

        // Report progress during tokenization
        if let Some(ref callback) = progress_callback {
            if (ord + 1) % report_interval == 0 || ord + 1 == total_sentences {
                callback(false, ord + 1, total_sentences);
            }
        }
    }

    // Phase 2: Process tokens in parallel
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };
    let processed_count = AtomicUsize::new(0);

    let all_terms: Vec<Vec<Term>> = sentence_tokens
        .par_iter()
        .map(|(_ord, _sentence_text, tokens)| {
            let words: Vec<Word> = match parse_into_words(tokens.clone()) {
                Ok(parsed_words) => parsed_words,
                Err(_) => Vec::new(),
            };

            let mut sentence_terms: Vec<Term> = words
                .into_iter()
                .filter_map(|word| {
                    let mut term: Term = word.into();

                    // Filter out blank/whitespace tokens
                    if term.surface_form.trim().is_empty() {
                        return None;
                    }

                    // For frequency analysis, use surface forms as lemma forms initially
                    // Deinflection will be applied in batch later
                    term.lemma_form = term.surface_form.clone();
                    term.lemma_reading = term.surface_reading.clone();

                    Some(term)
                })
                .collect();

            // Phrase detection - check if phrase exists in loaded dictionaries
            // Only add the largest matching phrase (no subphrases)
            'outer: for start in 0..sentence_terms.len() {
                for end in (start + 1..sentence_terms.len()).rev() {
                    let subrange = &sentence_terms[start..=end];
                    let mut phrase: Term = Term::from_slice(subrange);

                    let freq = frequency_manager.get_harmonic_frequency_for_pair(
                        &phrase.surface_form.normalize_long_vowel(),
                        &phrase.surface_reading.normalize_long_vowel(),
                    );

                    if let Some(_frequency) = freq {
                        let char_count = phrase.lemma_form.chars().count();
                        let min_len = 4;

                        if char_count < min_len {
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

                        sentence_terms.push(phrase);
                        break 'outer;
                    }
                }
            }

            // Update progress counter (phase = true)
            let current = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(ref callback) = progress_callback {
                if current % report_interval == 0 || current == total_sentences {
                    callback(true, current, total_sentences);
                }
            }

            sentence_terms
        })
        .collect();

    let terms: Vec<Term> = all_terms.into_iter().flatten().collect();

    terms
}

pub fn batch_deinflect_terms(
    terms: &[Term],
    frequency_manager: &FrequencyManager,
) -> HashMap<(String, String), (String, String)> {
    use std::collections::HashSet;

    use rayon::prelude::*;

    // Collect unique (surface_form, surface_reading, POS) tuples
    let mut unique_surfaces: HashSet<(String, String, POS)> = HashSet::new();
    for term in terms {
        if term.surface_form.as_str().is_japanese() {
            unique_surfaces.insert((
                term.surface_form.clone(),
                term.surface_reading.clone(),
                term.part_of_speech.clone(),
            ));
        }
    }

    // Deinflect unique surfaces
    let deinflection_map: HashMap<(String, String), (String, String)> = unique_surfaces
        .par_iter()
        .map(|(surface_form, surface_reading, pos)| {
            let (lemma_form, lemma_reading) = match pos {
                POS::Verb
                | POS::SuruVerb
                | POS::AdjectivalNoun
                | POS::Adjective
                | POS::NounExpression
                | POS::Expression
                | POS::Noun => {
                    let deinflections: Vec<(String, String)> =
                        pairwise_deinflection(surface_form, surface_reading);

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

                    if !sorted_deinflections.is_empty() {
                        sorted_deinflections[0].clone()
                    } else {
                        // No valid deinflection found, use surface form
                        (surface_form.clone(), surface_reading.clone())
                    }
                }
                _ => {
                    // Other POS types don't need deinflection
                    (surface_form.clone(), surface_reading.clone())
                }
            };

            ((surface_form.clone(), surface_reading.clone()), (lemma_form, lemma_reading))
        })
        .collect();

    deinflection_map
}

pub fn init_vibrato(
    dict_type: &DictType,
    progress_callback: Option<Box<dyn Fn(String) + Send>>,
) -> Result<Tokenizer, YomineError> {
    let dict_path = ensure_dictionary(dict_type, progress_callback)?;
    let dict = load_dictionary(dict_path.to_str().unwrap())?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}
