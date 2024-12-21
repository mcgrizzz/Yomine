use core::panic;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;
use std::{fs, io, path::Path};

use yomine::DictType;
use vibrato::tokenizer::worker::Worker;
use vibrato::{Dictionary, Tokenizer};

mod pos;

#[allow(dead_code)]

#[derive(Debug)]
enum InputType {
    Text,
    Srt,
    Epub,
}

pub trait Phrase {
    fn get_phrase(&self) -> &str;
}

struct Subtitle {
    index: u32, 
    time_stamp: String,
    line: String,
}

impl Phrase for Subtitle {
    fn get_phrase(&self) -> &str {
        &self.line
    }
}


struct ParsedFile<P: Phrase> {
    path: String,
    name: String,
    input_type: InputType,
    phrases: Vec<P>,
}

struct Word {
    word: String,
    base_form: String,
    reading: String,
    morphene_idx: u16,

}

struct Sentence {
    content: String,
}

impl Phrase for Sentence {
    fn get_phrase(&self) -> &str {
        &self.content
    }
}

fn main() {
    
    let pos_tree = match pos::load_tree() {
        Ok(pos_tree) => pos_tree,
        Err(e) => panic!("{e}")
    };

    //pos_tree.print_tree(0);

    let parsed_file = match read_srt("input/youtube.srt") {
        Ok(parsed) => parsed,
        Err(e) => panic!("{e}")
    };

    // let parsed_file = match read_txt("input/short_story.txt") {
    //     Ok(parsed) => parsed,
    //     Err(e) => panic!("{e}")
    // };

    let mut start = Instant::now();

    let tokenizer = match yomine::init_vibrato(DictType::Ipadic) {
        Ok(tokenizer) => tokenizer,
        Err(e) => panic!("{e}") 
    };

    let mut worker = tokenizer.new_worker();
    
    println!("Ready to tokenize: {:?}", start.elapsed());

    start = Instant::now();

    let extracted_words = extract_words(worker, parsed_file);

    println!("Tokenized: {:?}", start.elapsed());
}

fn extract_words(mut worker: Worker<'_>, parsed_file: ParsedFile<impl Phrase>) -> Vec<Word> {
    let words = Vec::<Word>::new();
    for phrase in &parsed_file.phrases {
        worker.reset_sentence(&phrase.get_phrase());
        worker.tokenize();

        println!("\ntext:\t{}", phrase.get_phrase());
        for token in worker.token_iter() {
            let details = token.feature();
            println!("token:\t{}\t{}", token.surface(), details);
        }
    }

    words
}

fn read_srt(path: &str)  -> Result<ParsedFile<Subtitle>, io::Error> {
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

fn read_txt(path: &str) -> Result<ParsedFile<Sentence>, io::Error> {
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