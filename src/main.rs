use std::{collections::{HashMap, HashSet}, sync::Arc};

use yomine::{anki::{AnkiState, FieldMapping}, core::{SourceFile, Term}, dictionary::DictType, frequency_dict, gui::YomineApp, parser::read_srt, pos, segmentation::{segmentator::{segment, SegmentationCache, Token}, tokenizer::{extract_words, init_vibrato}}};


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

    //temporary blacklist while I test and use application
    let blacklist = vec![
        "の",
        "を",
        "が",
        "と",
        "で",
        "だ",
        "も",
        "な",
        "お",
        "ん",
        "か",
        "れる",
        "です",
        "られる",
        "せる",
    ];

    let source_file = SourceFile {
        id: 3,
        source: "SRT".to_string(),
        title: "".to_string(),
        creator: None,
        original_file: "input/[erai-raws-timed]-sousou-no-frieren-S1E21.srt".to_string(),
    };

    let dict_type = DictType::Unidic;

    let sentences = read_srt(&source_file).expect("Failed to parse subtitles");
    let pos_lookup = pos::load_pos_lookup().expect("Failed to load POS");
    let tokenizer = init_vibrato(&dict_type).expect("Failed to initialize tokenizer");
    let frequency_manager = frequency_dict::process_frequency_dictionaries().expect("Failed to load Frequency Manager");

    // let mut cache = SegmentationCache::new();
    // let mut segment_terms: Vec<Term> = Vec::new();
    // for sentence in &sentences {
    //     let segs = segment(&sentence.text, &frequency_manager, &mut cache);
    //     let best_segs = segs.get_n_best_segments(1);

    //     for (_, segmentation) in best_segs.iter().enumerate() {
    //         let mut seg_tokens: Vec<Term> = segmentation.iter()
    //             .map(|s| {
    //                 let mut t = Term::from(s.token.clone());
    //                 let index_in_sentence = sentence
    //                     .text
    //                     .match_indices(&t.surface_form)
    //                     .next()
    //                     .map(|(idx, _)| idx)
    //                     .unwrap_or(0);
    //                 t.sentence_references = vec![(sentence.id, index_in_sentence)];
    //                 t
    //             }).collect();
    //         segment_terms.append(&mut seg_tokens);
    //     }
    // }
    
    let mut terms = extract_words(tokenizer.new_worker(), &sentences, &pos_lookup, &dict_type, &frequency_manager);

    // let found_terms: HashSet<String> = terms.iter().map(|t| t.lemma_form.clone()).collect();
    // terms.extend(segment_terms.into_iter().filter(|t| t.lemma_form.chars().count() > 3 && t.surface_form.chars().count() > 3 && !found_terms.contains(&t.lemma_form)));

    terms = terms.into_iter().filter(|term| !blacklist.contains(&term.lemma_form.as_str())).collect();
 
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