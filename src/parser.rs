use std::{
    fs,
    sync::LazyLock,
};

use regex::Regex;

use crate::core::{
    Sentence,
    SourceFile,
    YomineError,
};

static KANA_READING_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\(([^)]*)\)").expect("Failed to compile Japanese kana reading regex")
});

pub fn read_srt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    //So far we only know netflix uses this formatting as per (https://partnerhelp.netflixstudios.com/hc/en-us/articles/215767517-Japanese-Timed-Text-Style-Guide)
    let delete_readings = source_file.creator.as_deref() == Some("Netflix");

    let sentences: Vec<Sentence> = fs::read_to_string(&source_file.original_file)?
        .replace("\r", "")
        .split("\n\n")
        .filter(|s| !s.is_empty())
        .enumerate()
        .filter_map(|(id, entry)| {
            let lines: Vec<&str> = entry.trim().trim_start_matches("\n").splitn(3, "\n").collect();

            if lines.len() != 3 {
                return Some(Err(YomineError::Custom("Invalid subtitle format".to_string())));
            }

            let timestamp = lines[1].to_string();
            let raw_text = lines[2].to_string().replace("\n", ""); // Filter out newlines within the text.
            let mut text = raw_text;
            if delete_readings {
                text = KANA_READING_REGEX.replace_all(&text, "").trim().to_string()
            }

            if text.is_empty() {
                return None;
            }

            Some(Ok(Sentence {
                id: id as u32,
                source_id: source_file.id, // Reference to the SourceFile ID
                segments: vec![],          // segments are generated after tokenization
                text: text,
                timestamp: Some(timestamp),
            }))
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
            segments: vec![],          // segments are generated after tokenization
            text: s.to_string(),
            timestamp: None, // Text files don’t have timestamps
        })
        .collect();

    if sentences.is_empty() {
        return Err(YomineError::Custom("No sentences found in the file.".to_string()));
    }

    Ok(sentences)
}
