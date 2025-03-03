use std::collections::HashMap;

use vibrato::{tokenizer::worker::Worker, Tokenizer};
use crate::core::{Sentence, Term, YomineError};
use crate::dictionary::{ensure_dictionary, load_dictionary, DictType};
use crate::frequency_dict::FrequencyManager;
use crate::pos::PosLookup;
use wana_kana::IsJapaneseStr;

use super::token_models::{RawToken, UnidicToken, VibratoToken};
use super::unidic_tags::UnidicTag;


pub fn extract_words(mut worker: Worker<'_>, sentences: &[Sentence], pos_lookup: &PosLookup, dict_type: &DictType, frequency_manager: &FrequencyManager) -> Vec<Term> {
    let mut terms = Vec::<Term>::new();
    let mut term_id_counter = 1;

    for sentence in sentences.iter() {
        worker.reset_sentence(&sentence.text);
        worker.tokenize();

        
        for token in worker.token_iter() {
            let details = token.feature();
            let surface_form = token.surface().to_string();

            let indices = dict_type.lemma_indices();
            let lemma_form = details.split(',').nth(indices.0).unwrap_or("").to_string();
            let lemma_reading = details.split(',').nth(indices.1).unwrap_or("").to_string();

            // Convert the details into UnidicTag enums
            // let pos1: UnidicTag = details.split(',').nth(0).unwrap_or("").into();
            // let pos2: UnidicTag = details.split(',').nth(1).unwrap_or("").into();
            // let pos3: UnidicTag = details.split(',').nth(2).unwrap_or("").into();
            // let pos4: UnidicTag = details.split(',').nth(3).unwrap_or("").into();
            // let inflection_type: UnidicTag = details.split(',').nth(4).unwrap_or("").into(); //Called cType in unidic
            
            // Debugging output - can be removed or conditionally enabled
            // if inflection_type != UnidicTag::Unknown && inflection_type != UnidicTag::Unset {
                println!("{},{}", surface_form, details);
            // }

            let pos_key = details
                .split(',')
                .take(4) //Only four degrees of specification, otherwise the POS file grows too much
                .filter(|s| !(s.to_string() == "*")) 
                .collect::<Vec<_>>()
                .join(" -> "); // Combine with "->"


            // Determine if the surface form is written entirely in Kana
            let is_kana = surface_form.as_str().is_kana();

            // Find the position of the surface form in the sentence
            let index_in_sentence = sentence
                .text
                .match_indices(&surface_form)
                .next()
                .map(|(idx, _)| idx)
                .unwrap_or(0);

                
            let part_of_speech = pos_lookup.resolve(&pos_key);

            let freq_map: HashMap<String, u32> = frequency_manager.build_freq_map(&lemma_form, &lemma_reading, is_kana);
            
            let term = Term {
                id: 0,
                lemma_form,
                lemma_reading,
                surface_form,
                is_kana,
                part_of_speech,
                frequencies: freq_map, // Will be populated later
                sentence_references: vec![(sentence.id, index_in_sentence)],
            };
            
            terms.push(term);
        }
    }

    terms
}


pub fn init_vibrato(dict_type: &DictType) -> Result<Tokenizer, YomineError> {
    let dict_path = ensure_dictionary(dict_type)?;
    let dict = load_dictionary(dict_path.to_str().unwrap())?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}