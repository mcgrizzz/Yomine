use std::collections::HashSet;

use crate::core::{
    Sentence,
    SourceFile,
    Term,
};

pub struct FileData {
    pub source_file: SourceFile,
    pub processing_filename: Option<String>,

    // Extracted terms and sentences
    pub terms: Vec<Term>,
    pub original_terms: Vec<Term>,
    pub anki_filtered_terms: HashSet<String>,
    pub sentences: Vec<Sentence>,
    pub file_comprehension: f32,
}

impl FileData {
    pub fn has_terms(&self) -> bool {
        !self.terms.is_empty()
    }
}
