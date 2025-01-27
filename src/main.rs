use std::collections::HashMap;

use yomine::{anki::{get_models, get_total_vocab}, core::SourceFile, dictionary::DictType, frequency_dict, gui::YomineApp, parser::read_srt, pos, tokenizer::{extract_words, init_vibrato}};


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
    
    // let deck = Deck {
    //     name: "1: Daily Various".to_string(),
    //     id: 1706468857487,
    // };

    // let note_ids = match get_note_ids(deck).await {
    //     Err(err) => {
    //         Vec::new()
    //     },  
    //     Ok(note_ids) => {
    //         note_ids.get_ids()
    //     }
    // };

    // let notes = match get_notes(note_ids).await {
    //     Err(err) => {
    //         Vec::new()
    //     },  
    //     Ok(notes) => {
    //         notes.get_notes()
    //     }
    // };

    // let card_ids: Vec<u64> = notes.into_iter().map(|x| x.cards).flatten().collect();

    // let cards = match get_cards(card_ids).await {
    //     Err(err) => {
    //         println!("ERROR: {err}");
    //         Vec::new()
    //     },  
    //     Ok(cards) => {
    //         println!("Great success: {:?}", cards);
    //         cards.get_cards()
    //     }
    // };

    // let model_ids = match get_model_ids().await {
    //     Err(err) => {
    //         HashMap::new()
    //     },  
    //     Ok(models) => {
    //         models.get_model_ids()
    //     }
    // };

    // match get_field_names("Kaishi 1.5k".to_string()).await {
    //     Err(err) => {
    //         println!("ERROR: {err}");
    //     },  
    //     Ok(models) => {
    //         println!("Great success: {:?}", models);
    //     }
    // };

    match get_total_vocab().await {
        Err(err) => {
            println!("ERROR: {err}");
        },  
        Ok(vocabs) => {
            println!("Great success: {:?}", vocabs.into_iter().map(|vocab| vocab.term).collect::<Vec<String>>());
        }
    };



    // match get_decks().await {
    //     Err(err) => {
    //         println!("ERROR: {err}");
    //     }, 
    //     Ok(decks) => {
    //         println!("Great success: {:?}", decks);
    //     }
    // };

    // let native_options = eframe::NativeOptions::default();
    // let _ = eframe::run_native("Yomine App", native_options, Box::new(|cc| Ok(Box::new(YomineApp::new(cc, terms)))));
}