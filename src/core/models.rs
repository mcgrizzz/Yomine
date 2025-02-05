use std::collections::HashMap;


#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: u32,                // Unique identifier
    pub source: String,         // Source type (e.g., "YouTube", "Jimaku", "TXT")
    pub title: String,          // File name or descriptive title
    pub creator: Option<String>,// Optional creator information
    pub original_file: String,  // Path to the file stored locally
}

#[derive(Debug, Clone)]
pub struct Sentence {
    pub id: u32,                // Unique identifier
    pub source_id: u32,         // Reference to a SourceFile
    pub text: String,           // Sentence content
    pub timestamp: Option<String>, // Include timestamp for SRT files
}

#[derive(serde::Deserialize)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    pub lemma_form: String,               // Base form of the term aka lemma form.. what is found in a dictionary
    pub lemma_reading: String,           // Lemma form reading in hiragana (we will have convert from katakana)
    pub surface_form: String,            // How it is found in the sentence
    pub is_kana: bool,
    pub part_of_speech: PartOfSpeech,          // Grammatical category
    pub frequencies: HashMap<String, u32>,    // <(dictionary_id, frequency)>
    pub sentence_references: Vec<(u32, usize)>, // Vec<(sentence_id, index)>
}