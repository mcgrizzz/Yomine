use std::{fs, io, path::Path};

use crate::YomineError;

#[derive(Debug)]
enum InputType {
    Text,
    Srt,
    Epub,
}

pub trait Phrase {
    fn get_phrase(&self) -> &str;
}

pub struct Subtitle {
    index: u32, 
    time_stamp: String,
    line: String,
}

impl Phrase for Subtitle {
    fn get_phrase(&self) -> &str {
        &self.line
    }
}


pub struct ParsedFile<P: Phrase> {
    path: String,
    name: String,
    input_type: InputType,
    pub phrases: Vec<P>,
}

pub struct Word {
    word: String,
    base_form: String,
    reading: String,
    morphene_idx: u16,

}

pub struct Sentence {
    content: String,
}

impl Phrase for Sentence {
    fn get_phrase(&self) -> &str {
        &self.content
    }
}

pub fn read_srt(path: &str)  -> Result<ParsedFile<Subtitle>, YomineError> {
    let subs: Vec<Subtitle> = fs::read_to_string(path)?
        .split("\n\n")
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().split("\n").collect::<Vec<&str>>())
        .filter(|s| s.len() == 3)
        .map(|s| Subtitle {
            index: s[0].parse::<u32>().expect("Expected valid index"),
            time_stamp: s[1].to_string(), 
            line: s[2].to_string()}
        )
        .collect();

    if subs.len() == 0 {
        panic!("No subs");
    }
    let file_path = Path::new(path);
    let file_name = file_path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown File");

    Ok(ParsedFile {
        path: path.to_string(),
        name: file_name.to_string(),
        input_type: InputType::Srt,
        phrases: subs,
    })
}

pub fn read_txt(path: &str) -> Result<ParsedFile<Sentence>, YomineError> {
    let sentences: Vec<Sentence> = fs::read_to_string(path)?
        .split_terminator(['。', '！', '？', '「', '」', '\n'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| Sentence {content: s.to_string()})
        .collect();

    if sentences.len() == 0 {
        panic!("No sentences");
    }
    let file_path = Path::new(path);
    let file_name = file_path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown File");

    Ok(ParsedFile {
        path: path.to_string(),
        name: file_name.to_string(),
        input_type: InputType::Text,
        phrases: sentences,
    })
}