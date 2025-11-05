use core::fmt;
use std::collections::HashMap;

use wana_kana::IsJapaneseStr;

use super::{
    token_models::UnidicToken,
    unidic_tags::UnidicTag,
};
use crate::core::Term;

#[derive(PartialEq, Clone, Copy, Debug, Hash, Eq)]
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
    Postposition, // Like auxiliary verbs
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
    Expression,
    NounExpression,
    Other,
    Unknown,
}

impl fmt::Display for POS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl POS {
    pub fn display_name(&self) -> &'static str {
        match self {
            POS::Noun => "Noun",
            POS::ProperNoun => "Proper Noun",
            POS::CompoundNoun | POS::NounExpression => "Compound Noun",
            POS::Pronoun => "Pronoun",
            POS::Adjective => "Adjective",
            POS::AdjectivalNoun => "Adjectival Noun",
            POS::Adverb => "Adverb",
            POS::Determiner => "Determiner",
            POS::Preposition => "Preposition",
            POS::Postposition => "Particle",
            POS::Verb => "Verb",
            POS::SuruVerb => "Suru Verb",
            POS::Copula => "Copula",
            POS::Suffix => "Suffix",
            POS::Prefix => "Prefix",
            POS::Conjunction => "Conjunction",
            POS::Interjection => "Interjection",
            POS::Number => "Number",
            POS::Counter => "Counter",
            POS::Symbol => "Symbol",
            POS::Expression => "Expression",
            POS::Other => "Other",
            POS::Unknown => "Unknown",
        }
    }
    pub fn as_key(&self) -> &'static str {
        match self {
            POS::Noun => "Noun",
            POS::ProperNoun => "ProperNoun",
            POS::CompoundNoun => "CompoundNoun",
            POS::Pronoun => "Pronoun",
            POS::Adjective => "Adjective",
            POS::AdjectivalNoun => "AdjectivalNoun",
            POS::Adverb => "Adverb",
            POS::Determiner => "Determiner",
            POS::Preposition => "Preposition",
            POS::Postposition => "Postposition",
            POS::Verb => "Verb",
            POS::SuruVerb => "SuruVerb",
            POS::Copula => "Copula",
            POS::Suffix => "Suffix",
            POS::Prefix => "Prefix",
            POS::Conjunction => "Conjunction",
            POS::Interjection => "Interjection",
            POS::Number => "Number",
            POS::Counter => "Counter",
            POS::Symbol => "Symbol",
            POS::Expression => "Expression",
            POS::NounExpression => "NounExpression",
            POS::Other => "Other",
            POS::Unknown => "Unknown",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "Noun" => Some(POS::Noun),
            "ProperNoun" => Some(POS::ProperNoun),
            "CompoundNoun" => Some(POS::CompoundNoun),
            "Pronoun" => Some(POS::Pronoun),
            "Adjective" => Some(POS::Adjective),
            "AdjectivalNoun" => Some(POS::AdjectivalNoun),
            "Adverb" => Some(POS::Adverb),
            "Determiner" => Some(POS::Determiner),
            "Preposition" => Some(POS::Preposition),
            "Postposition" => Some(POS::Postposition),
            "Verb" => Some(POS::Verb),
            "SuruVerb" => Some(POS::SuruVerb),
            "Copula" => Some(POS::Copula),
            "Suffix" => Some(POS::Suffix),
            "Prefix" => Some(POS::Prefix),
            "Conjunction" => Some(POS::Conjunction),
            "Interjection" => Some(POS::Interjection),
            "Number" => Some(POS::Number),
            "Counter" => Some(POS::Counter),
            "Symbol" => Some(POS::Symbol),
            "Expression" => Some(POS::Expression),
            "NounExpression" => Some(POS::NounExpression),
            "Other" => Some(POS::Other),
            "Unknown" => Some(POS::Unknown),
            _ => None,
        }
    }
}

// impl fmt::Display for POS {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "{:?}", self) // Use Debug formatting as a placeholder
//     }
// }

pub struct Word {
    pub surface_form: String,
    pub surface_hatsuon: String, //hatsuon is easier to type than pronunciation...
    pub lemma_form: String,
    pub lemma_hatsuon: String,
    pub part_of_speech: POS,
    pub tokens: Vec<UnidicToken>,
    pub main_word: Option<UnidicToken>, // Optional core for the word (e.g., "勉強" in "勉強します")
}

impl From<Word> for Term {
    fn from(word: Word) -> Term {
        if let Some(main_word) = word.main_word {
            let is_kana = main_word.surface.as_str().is_kana();
            Term {
                id: 0,
                lemma_form: main_word.lemma_form,
                lemma_reading: main_word.lemma_hatsuon,
                surface_form: main_word.surface,
                surface_reading: main_word.surface_hatsuon.clone(),
                is_kana,
                part_of_speech: word.part_of_speech,
                full_segment: word.surface_form,
                full_segment_reading: word.surface_hatsuon,
                frequencies: HashMap::new(),
                sentence_references: Vec::new(),
                comprehension: 0.0,
            }
        } else {
            let is_kana = word.surface_form.as_str().is_kana();
            Term {
                id: 0,
                lemma_form: word.lemma_form,
                lemma_reading: word.lemma_hatsuon,
                surface_form: word.surface_form.clone(),
                surface_reading: word.surface_hatsuon.clone(),
                is_kana,
                part_of_speech: word.part_of_speech,
                full_segment: word.surface_form,
                full_segment_reading: word.surface_hatsuon,
                frequencies: HashMap::new(),
                sentence_references: Vec::new(),
                comprehension: 0.0,
            }
        }
    }
}

pub fn get_default_pos(token: &UnidicToken) -> POS {
    match token.pos1 {
        UnidicTag::Meishi => match token.pos2 {
            UnidicTag::Koyuumeishi => POS::ProperNoun,
            UnidicTag::Suushi => POS::Number,
            _ => POS::Noun,
        },
        UnidicTag::Doushi => POS::Verb,
        UnidicTag::Keiyoushi => POS::Adjective,
        UnidicTag::Keijoushi => POS::Adjective,
        UnidicTag::Fukushi => POS::Adverb,
        UnidicTag::Joshi => POS::Postposition,
        UnidicTag::Jodoushi => match token.conjugation_type {
            UnidicTag::JodoushiDesu => POS::Copula,
            _ => POS::Postposition,
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
