use std::collections::HashMap;

use vibrato::{
    tokenizer::worker::Worker,
    Tokenizer,
};
use wana_kana::IsJapaneseStr;

use super::{
    nbest::rescue_words,
    rule_matcher::parse_into_words,
    token_models::UnidicToken,
    word::{
        get_default_pos,
        Word,
        POS,
    },
};
use crate::{
    core::{
        utils::{
            is_kanji_char,
            normalize_reading,
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
            load_dictionary,
            DictType,
        },
    },
};

pub fn extract_words(
    mut worker: Worker,
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
                UnidicToken::from_parts(token.surface(), token.feature(), token.range_byte())
            })
            .collect();

        let words: Vec<Word> = match parse_into_words(tokens) {
            Ok(parsed_words) => parsed_words,
            Err(_) => Vec::new(),
        };
        let words = rescue_words(&mut worker, &sentence.text, words, frequency_manager);
        let words = split_unvalidated_compounds(words, frequency_manager);

        let mut term_spans: Vec<(usize, usize)> = Vec::with_capacity(words.len());
        let mut sentence_terms: Vec<Term> = Vec::with_capacity(words.len());
        for word in words {
            let span = word.byte_span();
            // The highlight span ends at start + surface_form.len(), so the
            // reference must point at the main word, not the segment.
            let ref_start = word.main_word.as_ref().map_or(span.0, |m| m.start_byte);
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
                            let unidic_lemma =
                                (term.lemma_form.clone(), term.lemma_reading.clone());
                            let chosen = sorted_deinflections
                                .iter()
                                .find(|candidate| **candidate == unidic_lemma)
                                .unwrap_or(&sorted_deinflections[0]);
                            term.lemma_form = chosen.0.clone();
                            term.lemma_reading = chosen.1.clone();
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

            term.sentence_references.push((ord, ref_start));
            term_spans.push(span);
            sentence_terms.push(term);
        }

        sentence.segments.extend(sentence_terms.iter().zip(&term_spans).map(
            |(term, &(start_index, end_index))| {
                // The span covers the FULL segment, so the reading must too:
                // `surface_reading` is the main word's alone (勉強します → べんきょう,
                // 8月 → がつ), which smeared a partial reading across the whole span
                // in furigana displays (the 8月22日 bug's second half).
                (
                    term.full_segment_reading.clone(),
                    term.part_of_speech.clone(),
                    start_index,
                    end_index,
                )
            },
        ));

        //Add phrases without filtering the terms for now
        for start in 0..sentence_terms.len() {
            for end in (start + 1..sentence_terms.len()).rev() {
                let subrange = &sentence_terms[start..=end];
                if !phrase_endpoint_ok(&subrange[0]) || !phrase_endpoint_ok(&subrange[end - start])
                {
                    continue;
                }
                let mut phrase: Term = Term::from_slice(subrange);

                // Try the plain component-concat reading first, then rendaku
                // variants (a non-initial component's first kana voiced:
                // 土曜+日 → どようひ, どようび). Dictionaries store the true
                // compound reading, and the pair lookup deliberately rejects a
                // form whose entries all carry a *different* reading — so
                // without the variants, no rendaku compound could ever be
                // promoted. A variant hit also corrects the phrase's own
                // reading for display/furigana.
                let mut freq = None;
                for candidate in phrase_reading_candidates(subrange) {
                    if let Some(f) = frequency_manager.get_harmonic_frequency_for_pair(
                        &phrase.surface_form.normalize_long_vowel(),
                        &candidate.normalize_long_vowel(),
                    ) {
                        phrase.surface_reading = candidate.clone();
                        phrase.lemma_reading = candidate.clone();
                        phrase.full_segment_reading = candidate;
                        freq = Some(f);
                        break;
                    }
                }

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

                    let all_nouns = subrange.iter().all(|term| {
                        matches!(
                            term.part_of_speech,
                            POS::Noun | POS::CompoundNoun | POS::ProperNoun
                        )
                    });

                    let score_threshold = 10.0;
                    let ratio_threshold = 120.0;
                    // The score/ratio gates guard against junk n-gram entries
                    // some dictionaries carry; all-kanji noun compounds with
                    // an exact reading match (複合体) don't need them.
                    let kanji_noun_compound =
                        all_nouns && phrase.lemma_form.chars().all(is_kanji_char);
                    let min_len = if kanji_noun_compound { 3 } else { 4 };

                    let override_ratio_threshold = 40.0;
                    let phrase_freq_threshold = 10000;

                    if char_count < min_len {
                        continue;
                    }

                    if !kanji_noun_compound
                        && frequency > phrase_freq_threshold
                        && max_ratio < override_ratio_threshold
                    {
                        continue;
                    }

                    phrase.part_of_speech =
                        if all_nouns { POS::NounExpression } else { POS::Expression };

                    if kanji_noun_compound
                        || score <= score_threshold
                        || max_ratio >= ratio_threshold
                    {
                        phrase.sentence_references.push((ord, term_spans[start].0));
                        let freq_map = frequency_manager.build_freq_map(
                            &phrase.lemma_form,
                            &phrase.lemma_reading,
                            phrase.is_kana,
                        );
                        phrase.frequencies = freq_map;
                        sentence_terms.push(phrase);

                        // Largest phrase at this start position is accepted; move to next start.
                        // (Was previously `break 'outer`, which stopped after the first phrase
                        // in the sentence — preventing detection of e.g. 土曜日 when an earlier
                        // 実は had already been accepted.)
                        break;
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
    tokenizer: &Tokenizer,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
    progress_callback: Option<&(dyn Fn(bool, usize, usize) + Sync)>,
) -> Vec<Term> {
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };

    use rayon::prelude::*;

    let total_sentences = sentences.len();
    let report_interval = std::cmp::min(1000, std::cmp::max(1, total_sentences / 20));
    let processed_count = AtomicUsize::new(0);

    let all_terms: Vec<Vec<Term>> = sentences
        .par_iter()
        .map_init(
            || tokenizer.new_worker(),
            |worker, sentence| {
                worker.reset_sentence(&sentence.text);
                worker.tokenize();

                let tokens: Vec<UnidicToken> = worker
                    .token_iter()
                    .map(|token| {
                        UnidicToken::from_parts(
                            token.surface(),
                            token.feature(),
                            token.range_byte(),
                        )
                    })
                    .collect();

                let words: Vec<Word> = match parse_into_words(tokens) {
                    Ok(parsed_words) => parsed_words,
                    Err(_) => Vec::new(),
                };
                let words = rescue_words(worker, &sentence.text, words, frequency_manager);
                let words = split_unvalidated_compounds(words, frequency_manager);

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
                // Adds the largest matching phrase at each starting position.
                for start in 0..sentence_terms.len() {
                    for end in (start + 1..sentence_terms.len()).rev() {
                        let subrange = &sentence_terms[start..=end];
                        if !phrase_endpoint_ok(&subrange[0])
                            || !phrase_endpoint_ok(&subrange[end - start])
                        {
                            continue;
                        }
                        let mut phrase: Term = Term::from_slice(subrange);

                        let freq = frequency_manager.get_harmonic_frequency_for_pair(
                            &phrase.surface_form.normalize_long_vowel(),
                            &phrase.surface_reading.normalize_long_vowel(),
                        );

                        if let Some(_frequency) = freq {
                            let char_count = phrase.lemma_form.chars().count();
                            let all_nouns = subrange.iter().all(|term| {
                                matches!(
                                    term.part_of_speech,
                                    POS::Noun | POS::CompoundNoun | POS::ProperNoun
                                )
                            });
                            let min_len =
                                if all_nouns && phrase.lemma_form.chars().all(is_kanji_char) {
                                    3
                                } else {
                                    4
                                };

                            if char_count < min_len {
                                continue;
                            }

                            phrase.part_of_speech =
                                if all_nouns { POS::NounExpression } else { POS::Expression };

                            sentence_terms.push(phrase);
                            // Move to next start position (was `break 'outer`).
                            break;
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
            },
        )
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

fn phrase_endpoint_ok(term: &Term) -> bool {
    !matches!(term.part_of_speech, POS::Postposition | POS::Copula | POS::Symbol)
}

/// Split rule-merged proper+common compounds back apart (夏子おばさん →
/// 夏子 + おばさん) unless a frequency dictionary corroborates the merged pair.
fn split_unvalidated_compounds(
    words: Vec<Word>,
    frequency_manager: &FrequencyManager,
) -> Vec<Word> {
    if frequency_manager.get_enabled_dictionaries().is_empty() {
        return words;
    }
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for word in words {
        if word.part_of_speech != POS::CompoundNoun
            || word.tokens.len() < 2
            || frequency_manager
                .get_harmonic_frequency_for_pair(&word.surface_form, &word.surface_hatsuon)
                .is_some()
        {
            out.push(word);
            continue;
        }
        let Ok(rest_words) = parse_into_words(word.tokens[1..].to_vec()) else {
            out.push(word);
            continue;
        };
        let first = &word.tokens[0];
        out.push(Word {
            surface_form: first.surface.clone(),
            surface_hatsuon: normalize_reading(&first.surface, &first.surface_hatsuon),
            lemma_form: first.lemma_form.clone(),
            lemma_hatsuon: normalize_reading(&first.surface, &first.lemma_hatsuon),
            part_of_speech: get_default_pos(first),
            tokens: vec![first.clone()],
            main_word: None,
        });
        out.extend(rest_words);
    }
    out
}

/// Dakuten/handakuten variants of a leading kana — the sequential-voicing
/// (rendaku) alternations a non-initial compound component can take.
fn rendaku_variants(c: char) -> &'static [char] {
    match c {
        'か' => &['が'],
        'き' => &['ぎ'],
        'く' => &['ぐ'],
        'け' => &['げ'],
        'こ' => &['ご'],
        'さ' => &['ざ'],
        'し' => &['じ'],
        'す' => &['ず'],
        'せ' => &['ぜ'],
        'そ' => &['ぞ'],
        'た' => &['だ'],
        'ち' => &['ぢ'],
        'つ' => &['づ'],
        'て' => &['で'],
        'と' => &['ど'],
        'は' => &['ば', 'ぱ'],
        'ひ' => &['び', 'ぴ'],
        'ふ' => &['ぶ', 'ぷ'],
        'へ' => &['べ', 'ぺ'],
        'ほ' => &['ぼ', 'ぽ'],
        _ => &[],
    }
}

/// Candidate readings for a compound: the plain component concat first, then —
/// one boundary at a time — the same reading with an interior component's first
/// kana voiced (土曜+日 → [どようひ, どようび, どようぴ]).
fn phrase_reading_candidates(subrange: &[Term]) -> Vec<String> {
    let base: String = subrange.iter().map(|t| t.full_segment_reading.as_str()).collect();
    let base_chars: Vec<char> = base.chars().collect();
    let mut candidates = vec![base];

    let mut offset = subrange[0].full_segment_reading.chars().count();
    for term in &subrange[1..] {
        let len = term.full_segment_reading.chars().count();
        if len > 0 {
            if let Some(&first) = base_chars.get(offset) {
                for &variant in rendaku_variants(first) {
                    let mut chars = base_chars.clone();
                    chars[offset] = variant;
                    candidates.push(chars.into_iter().collect());
                }
            }
        }
        offset += len;
    }
    candidates
}

pub fn init_vibrato(
    dict_type: &DictType,
    progress_callback: Option<Box<dyn Fn(String) + Send>>,
) -> Result<Tokenizer, YomineError> {
    let dict = load_dictionary(dict_type, progress_callback)?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}
