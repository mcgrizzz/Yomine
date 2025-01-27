use std::fs;

use crate::core::{Sentence, SourceFile, YomineError};

pub fn read_srt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let sentences: Vec<Sentence> = fs::read_to_string(&source_file.original_file)?
        .split("\n\n")
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(id, entry)| {
            let lines: Vec<&str> = entry.trim().split("\n").collect();
            if lines.len() != 3 {
                return Err(YomineError::Custom("Invalid subtitle format".to_string()));
            }

            Ok(Sentence {
                id: id as u32,
                source_id: source_file.id, // Reference to the SourceFile ID
                text: lines[2].to_string(),
                timestamp: Some(lines[1].to_string()),
            })
        })
        .collect::<Result<Vec<_>, YomineError>>()?;

    if sentences.is_empty() {
        return Err(YomineError::Custom("No subtitles found in the file.".to_string()));
    }

    Ok(sentences)
}

pub fn read_txt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let sentences: Vec<Sentence> = fs::read_to_string(&source_file.original_file)?
        .split_terminator(['。', '！', '？', '\n'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(id, s)| Sentence {
            id: id as u32,
            source_id: source_file.id, // Reference to the SourceFile ID
            text: s.to_string(),
            timestamp: None, // Text files don’t have timestamps
        })
        .collect();

    if sentences.is_empty() {
        return Err(YomineError::Custom("No sentences found in the file.".to_string()));
    }

    Ok(sentences)
}