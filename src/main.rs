use std::collections::HashMap;

use yomine::{anki::{api::get_version, get_models, get_total_vocab, wait_awake}, core::SourceFile, dictionary::DictType, frequency_dict, gui::YomineApp, parser::read_srt, pos, tokenizer::{extract_words, init_vibrato}};


#[tokio::main]
async fn main() {
    // Load subtitles and tokenize terms
    // let source_file = SourceFile {
    //     id: 1,
    //     source: "SRT".to_string(),
    //     title: "Example Subtitle".to_string(),
    //     creator: None,
    //     original_file: "input/youtube.srt".to_string(),
    // };
    
    // let dict_type = DictType::Unidic;

    // let sentences = read_srt(&source_file).expect("Failed to parse subtitles");
    // let pos_lookup = pos::load_pos_lookup().expect("Failed to load POS");
    // let tokenizer = init_vibrato(&dict_type).expect("Failed to initialize tokenizer");
    // let frequency_manager = frequency_dict::process_frequency_dictionaries().expect("Failed to load Frequency Manager");
    // let terms = extract_words(tokenizer.new_worker(), &sentences, &pos_lookup, &dict_type, &frequency_manager);
    

    // match get_total_vocab().await {
    //     Err(err) => {
    //         println!("ERROR: {err}");
    //     },  
    //     Ok(vocabs) => {
    //         println!("Great success: {:?}", vocabs.into_iter().map(|vocab| vocab.term).collect::<Vec<String>>());
    //     }
    // };

    match wait_awake(5, 5).await {
        Err(err) => {
            println!("ERROR: {err}");
        },  
        Ok(online) => {
            println!("Great success: {:?}", online);
        }
    };

    // let native_options = eframe::NativeOptions::default();
    // let _ = eframe::run_native("Yomine App", native_options, Box::new(|cc| Ok(Box::new(YomineApp::new(cc, terms)))));
}