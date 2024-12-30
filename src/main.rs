use core::panic;
use std::time::Instant;

use yomine::{dictionary::DictType, parser, pos, frequency_dict};

fn main() {

    let pos_tree = match pos::load_tree() {
        Ok(pos_tree) => pos_tree,
        Err(e) => panic!("{e}")
    };

    //pos_tree.print_tree(0);

    let parsed_file = match parser::read_srt("input/youtube.srt") {
        Ok(parsed) => parsed,
        Err(e) => panic!("{e}")
    };

    // let parsed_file = match read_txt("input/short_story.txt") {
    //     Ok(parsed) => parsed,
    //     Err(e) => panic!("{e}")
    // };

    let mut start = Instant::now();

    let tokenizer = match yomine::tokenizer::init_vibrato(DictType::Ipadic) {
        Ok(tokenizer) => tokenizer,
        Err(e) => panic!("{e}") 
    };

    let mut worker = tokenizer.new_worker();
    
    println!("Ready to tokenize: {:?}", start.elapsed());

    start = Instant::now();

    let extracted_words = yomine::tokenizer::extract_words(worker, parsed_file);

    println!("Tokenized: {:?}", start.elapsed());

    start = Instant::now();

    let dictionaries = match frequency_dict::process_frequency_dictionaries() {
        Ok(dictionaries) => dictionaries,
        Err(e) => panic!("{e}") 
    };

    println!("Loaded: {:?} frequency dictionaries in {:?}", dictionaries.len(), start.elapsed());
}