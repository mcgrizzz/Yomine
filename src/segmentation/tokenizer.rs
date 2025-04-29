use std::collections::HashMap;

use vibrato::{tokenizer::worker::Worker, Tokenizer};
use crate::core::utils::pairwise_deinflection;
use crate::core::{Sentence, Term, YomineError};
use crate::dictionary::{ensure_dictionary, load_dictionary, DictType};
use crate::frequency_dict::FrequencyManager;
use crate::pos::PosLookup;
use jp_deinflector::deinflect;
use wana_kana::IsJapaneseStr;

use super::rule_matcher::parse_into_words;
use super::token_models::{RawToken, UnidicToken, VibratoToken};
use super::word::{Word, POS};


pub fn extract_words(mut worker: Worker<'_>, sentences: &mut [Sentence], pos_lookup: &PosLookup, dict_type: &DictType, frequency_manager: &FrequencyManager) -> Vec<Term> {
    let mut terms = Vec::<Term>::new();
    //let mut term_id_counter = 1;

    for sentence in sentences.iter_mut() {
        worker.reset_sentence(&sentence.text);
        worker.tokenize();
        
        let tokens: Vec<UnidicToken> = worker.token_iter()
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

        let mut sentence_terms: Vec<Term> = words.into_iter()
            .map(|word| {
                let mut term: Term = word.into();
                if term.surface_form.as_str().is_japanese() {
                    match term.part_of_speech {
                        POS::Verb | POS::SuruVerb | POS::AdjectivalNoun | POS::Adjective | POS::Noun => {
                            let deinflections: Vec<(String, String)> = pairwise_deinflection(&term.surface_form, &term.surface_reading);
    
                            // println!("\nOriginal form: {}/{}", term.surface_form, term.surface_reading);
                            // println!("Before filter -- Deinflections = {:?}", deinflections);

                            let mut sorted_deinflections: Vec<(String, String)> = deinflections
                                .into_iter()
                                .filter(|(word, reading)| frequency_manager.get_harmonic_frequency_for_pair(word, reading).is_some())
                                .collect();

                            sorted_deinflections.sort_by_key(|(word, reading)| frequency_manager.get_harmonic_frequency_for_pair(word, reading));

                            if sorted_deinflections.len() > 0 {
                                term.lemma_form = sorted_deinflections[0].0.clone();
                                term.lemma_reading = sorted_deinflections[0].1.clone();
                            }
                            // Debug output
                            
                            // for (word, reading) in &sorted_deinflections {
                            //     println!("Deinflected Pair: Word = {}, Reading = {}, Harmonic Frequency = {:?}", word, reading, frequency_manager.get_harmonic_frequency_for_pair(word, reading));
                            // }
                        },
                        _ => {}
                    }
                }
                
                let freq_map: HashMap<String, u32> = frequency_manager.build_freq_map(&term.lemma_form, &term.lemma_reading, term.is_kana);
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

        sentence.segments.extend(
            sentence_terms.iter().map(|term| {
                let start_index = sentence
                    .text
                    .match_indices(&term.full_segment)
                    .next()
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                let end_index = start_index + term.full_segment.len();
                (term.surface_reading.clone(), term.part_of_speech.clone(), start_index, end_index)
            })
        );

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