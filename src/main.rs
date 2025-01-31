use std::{collections::HashMap, sync::Arc};

use wana_kana::ConvertJapanese;
use yomine::{anki::{api::get_version, get_models, get_total_vocab, wait_awake, AnkiState, FieldMapping}, core::{SourceFile}, dictionary::DictType, frequency_dict, gui::YomineApp, parser::read_srt, pos, tokenizer::{extract_words, init_vibrato}};


#[tokio::main]
async fn main() {
    //Load subtitles and tokenize terms
    // let source_file = SourceFile {
    //     id: 1,
    //     source: "SRT".to_string(),
    //     title: "Example Subtitle".to_string(),
    //     creator: None,
    //     original_file: "input/youtube.srt".to_string(),
    // };

    // let source_file = SourceFile {
    //     id: 2,
    //     source: "SRT".to_string(),
    //     title: "【Japanese Talk ＃6】私が転職した理由について話します".to_string(),
    //     creator: Some("あかね的日本語教室".to_string()),
    //     original_file: "input/【Japanese Talk ＃6】私が転職した理由について話します.srt".to_string(),
    // };

    //ダンダダン.S01E08.なんかモヤモヤするじゃんよ.WEBRip.Netflix.ja[cc].srt
    let source_file = SourceFile {
        id: 3,
        source: "SRT".to_string(),
        title: "Dan Da Dan - S01E08".to_string(),
        creator: None,
        original_file: "input/ダンダダン.S01E08.なんかモヤモヤするじゃんよ.WEBRip.Netflix.ja[cc].srt".to_string(),
    };

    let dict_type = DictType::Unidic;

    let sentences = read_srt(&source_file).expect("Failed to parse subtitles");
    let pos_lookup = pos::load_pos_lookup().expect("Failed to load POS");
    let tokenizer = init_vibrato(&dict_type).expect("Failed to initialize tokenizer");
    let frequency_manager = frequency_dict::process_frequency_dictionaries().expect("Failed to load Frequency Manager");
    let mut terms = extract_words(tokenizer.new_worker(), &sentences, &pos_lookup, &dict_type, &frequency_manager);
    
 
    let mut model_mapping: HashMap<String, FieldMapping> = HashMap::new();
    model_mapping.insert(
        "Lapis".to_string(), 
        FieldMapping {
            term_field: "Expression".to_string(),
            reading_field: "ExpressionReading".to_string(),
        });

        model_mapping.insert(
            "Kaishi 1.5k".to_string(), 
            FieldMapping {
                term_field: "Word".to_string(),
                reading_field: "Word Reading".to_string(),
            });

    let anki_state = match AnkiState::new(model_mapping, Arc::new(frequency_manager)).await {
        Err(err) => {
            println!("Unable to load AnkiState");
            None
        },  
        Ok(state) => {
            println!("Loaded AnkiState");
            Some(state)
        }
    };


    if let Some(state) = anki_state {
        println!("Prefiltered: {}", terms.len());
        terms = state.filter_existing_terms(terms, false);
        println!("Filtered: {}", terms.len());
        terms = state.filter_existing_terms(terms, true);
        println!("Filtered surface form: {}", terms.len());
    }


    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native("Yomine App", native_options, Box::new(|cc| Ok(Box::new(YomineApp::new(cc, terms, sentences)))));
}