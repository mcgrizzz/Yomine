use std::{
    collections::HashMap,
    hash::Hash,
};

use crate::segmentation::word::POS;

#[derive(Debug, Clone)]
pub enum FileType {
    SRT,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: u32,                 // Unique identifier
    pub source: String,          // Source type (e.g., "YouTube", "Jimaku", "TXT")
    pub file_type: FileType,     // File type (e.g., SRT, TXT)
    pub title: String,           // File name or descriptive title
    pub creator: Option<String>, // Optional creator information
    pub original_file: String,   // Path to the file stored locally
}

#[derive(Debug, Clone)]
pub struct Sentence {
    pub id: u32,                                    // Unique identifier
    pub source_id: u32,                             // Reference to a SourceFile
    pub text: String,                               // Sentence content
    pub segments: Vec<(String, POS, usize, usize)>, // List of segments (reading, POS, start, end) for the sentence
    pub timestamp: Option<String>,                  // Include timestamp for SRT files
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartOfSpeech {
    pub key: String,
    pub english_name: String,
    pub hint: String,
    pub examples: Vec<String>,
}

impl PartOfSpeech {
    pub fn new(key: String) -> Self {
        PartOfSpeech {
            key: key.clone(),
            english_name: key,
            hint: "".to_string(),
            examples: Vec::new(),
        }
    }

    pub fn is_verb(&self) -> bool {
        return self.key.starts_with("動詞");
    }

    pub fn is_i_adjective(&self) -> bool {
        return self.key.starts_with("形容詞");
    }
}

#[derive(Debug, Clone)]
pub struct Term {
    pub id: u32,
    pub lemma_form: String, // Base form of the term aka lemma form.. what is found in a dictionary
    pub lemma_reading: String, // Lemma form reading in hiragana (we will have convert from katakana)
    pub surface_form: String,  // How it is found in the sentence
    pub surface_reading: String,
    pub is_kana: bool,
    pub part_of_speech: POS,                    // Grammatical category
    pub frequencies: HashMap<String, u32>,      // <(dictionary_id, frequency)>
    pub full_segment: String, //If we have main word, this includes the non-main part of the segment, in surface form
    pub full_segment_reading: String, //If we have main word, this includes the non-main part of the segment, in surface form
    pub sentence_references: Vec<(u32, usize)>, // Vec<(sentence_id, start_index)>
}

impl Term {
    //Generate a phrase from a slice of terms
    pub fn from_slice(terms: &[Term]) -> Self {
        let full_segment = terms.iter().map(|t| t.full_segment.as_str()).collect::<String>();
        let full_segment_reading = terms.iter().map(|t| t.full_segment_reading.as_str()).collect::<String>();
        let surface_form = full_segment.clone();
        let surface_reading = full_segment_reading.clone();
        let lemma_form = surface_form.clone();
        let lemma_reading = surface_reading.clone();
        let is_kana = terms.iter().all(|t| t.is_kana);
        Term {
            id: 1,
            surface_form,
            surface_reading,
            lemma_form,
            lemma_reading,
            is_kana,
            part_of_speech: POS::Expression,
            full_segment,
            full_segment_reading,
            frequencies: HashMap::new(),
            sentence_references: Vec::new(),
        }
    }
}
