use std::{
    collections::HashMap,
    hash::Hash,
};

use time::Time;

use crate::segmentation::word::POS;

#[derive(Debug, Clone)]
pub enum SourceFileType {
    SRT,
    SSA,
    Other(String),
}

impl SourceFileType {
    pub fn from_extension(file_path: &str) -> Self {
        if let Some(extension) =
            std::path::Path::new(file_path).extension().and_then(|ext| ext.to_str())
        {
            match extension.to_lowercase().as_str() {
                "srt" => SourceFileType::SRT,
                "ass" | "ssa" => SourceFileType::SSA,
                other => SourceFileType::Other(other.to_uppercase()),
            }
        } else {
            SourceFileType::SRT
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: u32,                   // Unique identifier
    pub source: Option<String>,    // Source type (e.g., "YouTube", "Jimaku", "TXT")
    pub file_type: SourceFileType, // File type (e.g., SRT, TXT)
    pub title: String,             // File name or descriptive title
    pub creator: Option<String>,   // Optional creator information
    pub original_file: String,     // Path to the file
}

impl Default for SourceFile {
    fn default() -> Self {
        Self {
            id: 0,
            source: None,
            file_type: SourceFileType::SRT,
            title: String::new(),
            creator: None,
            original_file: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeStamp {
    pub start: Time,
    pub end: Time,
}

impl TimeStamp {
    pub fn to_secs(&self) -> (f32, f32) {
        fn secs(t: Time) -> f32 {
            let (h, m, s, ms) = t.as_hms_milli();
            (h as f32) * 3600.0 + (m as f32) * 60.0 + (s as f32) + (ms as f32) / 1000.0
        }

        (secs(self.start), secs(self.end))
    }

    pub fn to_human_readable(&self) -> (String, String) {
        fn fmt_time(t: Time) -> String {
            let (h, m, s, _ms) = t.as_hms_milli();

            if h > 0 {
                format!("{}h {}m {}s", h, m, s)
            } else if m > 0 {
                format!("{}m {}s", m, s)
            } else {
                format!("{}s", s)
            }
        }

        let start_str = fmt_time(self.start);
        let stop_str = fmt_time(self.end);

        (format!("{:<11}", start_str), format!("{:<11}", stop_str))
    }
}

#[derive(Debug, Clone)]
pub struct Sentence {
    pub id: usize,                                  // Unique identifier
    pub source_id: u32,                             // Reference to a SourceFile
    pub text: String,                               // Sentence content
    pub segments: Vec<(String, POS, usize, usize)>, // List of segments (reading, POS, start, end) for the sentence
    pub timestamp: Option<TimeStamp>,
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
    pub part_of_speech: POS,                      // Grammatical category
    pub frequencies: HashMap<String, u32>,        // <(dictionary_id, frequency)>
    pub full_segment: String, //If we have main word, this includes the non-main part of the segment, in surface form
    pub full_segment_reading: String, //If we have main word, this includes the non-main part of the segment, in surface form
    pub sentence_references: Vec<(usize, usize)>, // Vec<(sentence_id, start_index)>
}

impl Term {
    //Generate a phrase from a slice of terms
    pub fn from_slice(terms: &[Term]) -> Self {
        let full_segment = terms.iter().map(|t| t.full_segment.as_str()).collect::<String>();
        let full_segment_reading =
            terms.iter().map(|t| t.full_segment_reading.as_str()).collect::<String>();
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
