use std::{
    fs,
    sync::LazyLock,
};

use regex::Regex;
use rsubs_lib::SRT;

use crate::core::{
    models::{
        SourceFileType,
        TimeStamp,
    },
    Sentence,
    SourceFile,
    YomineError,
};

// Regex now handles any parathesis (full or half width) that contains only hiragana. Not sure if we should include Katakana
// but that would easy to add, just add '\p{scx=Katakana}'
static KANA_READING_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\(|（)[\p{scx=Hiragana}・･\s]+(?:\)|）)")
        .expect("Failed to compile kana-reading regex")
});

pub fn read_srt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    //So far we only know netflix uses this formatting as per (https://partnerhelp.netflixstudios.com/hc/en-us/articles/215767517-Japanese-Timed-Text-Style-Guide)
    // let delete_readings = source_file.creator.as_deref() == Some("Netflix");
    let delete_readings = true;

    let raw_srt = fs::read_to_string(&source_file.original_file)?;
    //new rsubs doesn't like utf-8 BOM at the beginning of the file:
    let raw_srt = raw_srt.trim_start_matches('\u{feff}');
    let srt = SRT::parse(raw_srt)
        .map_err(|err| YomineError::Custom(format!("Error Parsing SRT File: {}", err)))?;

    let sentences: Vec<Sentence> = srt
        .lines
        .iter()
        .filter(|s| !s.text.is_empty())
        .enumerate()
        .filter_map(|(id, entry)| {
            let raw_text = entry.text.replace("\n", "");

            let mut text = raw_text;
            if delete_readings {
                text = KANA_READING_REGEX.replace_all(&text, "").trim().to_string()
            }

            if text.is_empty() {
                return None;
            }

            let timestamp = TimeStamp { start: entry.start, end: entry.end };

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

fn read_ass(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    todo!()
}

pub fn read(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    match source_file.file_type {
        SourceFileType::SRT => read_srt(source_file),
        SourceFileType::ASS => read_ass(source_file),
        SourceFileType::Other(ref format) => Err(YomineError::UnsupportedFileType(format.clone())),
    }
}
