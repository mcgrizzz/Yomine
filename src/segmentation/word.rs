use super::token_models::UnidicToken;
use super::unidic_tags::UnidicTag;

#[derive(PartialEq, Clone, Debug)]
pub enum POS {
    Noun,
    ProperNoun,
    Pronoun,
    Adjective,
    Adverb,
    Determiner,
    Preposition, 
    Postposition, //Like auxillary verbs
    Verb,
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

pub struct Word {
    pub surface_form: String, 
    pub surface_hatsuon: String, //hatsuon is easier to type than pronunciation...
    pub lemma_form: String,
    pub lemma_hatsuon: String,
    pub part_of_speech: POS,
    pub tokens: Vec<UnidicToken>,
    pub main_word: Option<String>, // Optional core for the word (e.g., "勉強" in "勉強します")
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
        UnidicTag::Jodoushi => POS::Postposition,
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