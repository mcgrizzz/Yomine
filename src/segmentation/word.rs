use core::fmt;
use std::collections::HashMap;

use wana_kana::IsJapaneseStr;

use crate::core::Term;

use super::token_models::UnidicToken;
use super::unidic_tags::UnidicTag;

#[derive(PartialEq, Clone, Debug)]
pub enum POS {
    Noun,
    ProperNoun,
    CompoundNoun,
    Pronoun,
    Adjective,
    AdjectivalNoun,
    Adverb,
    Determiner,
    Preposition, 
    Postposition, //Like auxillary verbs
    Verb,
    SuruVerb,
    Copula,
    Suffix,
    Prefix,
    Conjunction,
    Interjection, 
    Number,
    Counter,
    Symbol,
    Other,
    Unknown,
}

impl fmt::Display for POS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self) // Use Debug formatting as a placeholder
    }
}

pub struct Word {
    pub surface_form: String, 
    pub surface_hatsuon: String, //hatsuon is easier to type than pronunciation...
    pub lemma_form: String,
    pub lemma_hatsuon: String,
    pub part_of_speech: POS,
    pub tokens: Vec<UnidicToken>,
    pub main_word: Option<UnidicToken>, // Optional core for the word (e.g., "勉強" in "勉強します")
}

impl Into<Term> for Word {
    fn into(self) -> Term {
        if let Some(main_word) = self.main_word {
            Term {
                id: 0,
                lemma_form: main_word.lemma_form,
                lemma_reading: main_word.lemma_hatsuon,
                surface_form: main_word.surface,
                surface_reading: main_word.surface_hatsuon.clone(),
                is_kana: main_word.surface_hatsuon.as_str().is_kana(),
                part_of_speech: self.part_of_speech,
                full_segment: self.surface_form,
                full_segment_reading: self.surface_hatsuon,
                frequencies: HashMap::new(),
                sentence_references: Vec::new(),
            }
        } else {
            Term {
                id: 0,
                lemma_form: self.lemma_form,
                lemma_reading: self.lemma_hatsuon,
                surface_form: self.surface_form.clone(),
                surface_reading: self.surface_hatsuon.clone(),
                is_kana: self.surface_hatsuon.as_str().is_kana(),
                part_of_speech: self.part_of_speech,
                full_segment: self.surface_form,
                full_segment_reading: self.surface_hatsuon,
                frequencies: HashMap::new(),
                sentence_references: Vec::new(),
            }
        }
    }
}

pub fn get_default_pos(token: &UnidicToken) -> POS {
    match token.pos1 {
        UnidicTag::Meishi => {
            match token.pos2 {
                UnidicTag::Koyuumeishi => POS::ProperNoun,
                UnidicTag::Suushi => POS::Number,
                _ => POS::Noun,
            }
        },
        UnidicTag::Doushi => POS::Verb,
        UnidicTag::Keiyoushi => POS::Adjective,
        UnidicTag::Keijoushi => POS::Adjective,
        UnidicTag::Fukushi => POS::Adverb,
        UnidicTag::Joshi => POS::Postposition,
        UnidicTag::Jodoushi => {
            match token.conjugation_type {
                UnidicTag::JodoushiDesu => POS::Copula,
                _ => POS::Postposition,
            }
            
        },
        UnidicTag::Rentaishi => POS::Determiner,
        UnidicTag::Setsuzokushi => POS::Conjunction,
        UnidicTag::Settouji => POS::Prefix,
        UnidicTag::Setsubiji => POS::Suffix,
        UnidicTag::Kigou => POS::Symbol,
        UnidicTag::Daimeshi => POS::Pronoun,
        UnidicTag::Kandoushi | UnidicTag::Firaa => POS::Interjection,
        UnidicTag::Hojokigou => POS::Symbol,
        _ => POS::Other,
    }

}