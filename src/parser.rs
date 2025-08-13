use std::{
    fs,
    sync::LazyLock,
};

use regex::Regex;
use rsubs_lib::{
    SRT,
    SSA,
};

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

static STRIP_INLINE_TAGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)</?(?:b|i|u)>").expect("Failed to compile inline_strip_tags regex")
});

fn parse_srt(srt: SRT, source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let sentences: Vec<Sentence> = srt
        .lines
        .iter()
        .filter(|s| !s.text.is_empty())
        .enumerate()
        .filter_map(|(id, entry)| {
            let raw_text = entry.text.split_whitespace().collect::<Vec<_>>().join(" ");

            let text = raw_text;
            let text = KANA_READING_REGEX.replace_all(&text, "");
            let text = STRIP_INLINE_TAGS.replace_all(&text, "").to_string();

            if text.is_empty() {
                return None;
            }

            let timestamp = TimeStamp { start: entry.start, end: entry.end };

            Some(Ok(Sentence {
                id: id,
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

pub fn read_srt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    //So far we only know netflix uses this formatting as per (https://partnerhelp.netflixstudios.com/hc/en-us/articles/215767517-Japanese-Timed-Text-Style-Guide)
    // let delete_readings = source_file.creator.as_deref() == Some("Netflix");

    let raw_srt = fs::read_to_string(&source_file.original_file)?;
    //new rsubs doesn't like utf-8 BOM at the beginning of the file:
    let raw_srt = raw_srt.trim_start_matches('\u{feff}');
    let srt = SRT::parse(raw_srt)
        .map_err(|err| YomineError::Custom(format!("Error Parsing SRT File: {}", err)))?;

    parse_srt(srt, source_file)
}

fn read_ssa(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let raw_file = fs::read_to_string(&source_file.original_file)?;
    let raw_file = raw_file.trim_start_matches('\u{feff}');

    let ssa = SSA::parse_lenient(raw_file)
        .map_err(|err| YomineError::Custom(format!("Error Parsing SSA/ASS File: {}", err)))?;

    let srt = ssa.to_srt();

    parse_srt(srt, source_file)
}

pub fn read(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    match source_file.file_type {
        SourceFileType::SRT => read_srt(source_file),
        SourceFileType::SSA => read_ssa(source_file),
        SourceFileType::Other(ref format) => Err(YomineError::UnsupportedFileType(format.clone())),
    }
}
