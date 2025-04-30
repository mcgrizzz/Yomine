use std::{collections::HashMap, sync::Arc};

use yomine::{anki::FieldMapping, core::{models::FileType, pipeline::process_source_file, SourceFile}, dictionary::DictType, frequency_dict, gui::{LanguageTools, YomineApp}, segmentation::tokenizer::init_vibrato};


#[tokio::main]
async fn main() {
    let source_file = SourceFile {
        id: 3,
        source: "SRT".to_string(),
        file_type: FileType::SRT,
        title: "".to_string(),
        creator: None,
        original_file: "input/[Japanese] 空港直結の最高級カプセルホテルに宿泊、一泊12,000円はさすがに... [DownSub.com].srt".to_string(),
    };

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

    let dict_type = DictType::Unidic;
    let tokenizer = Arc::new(init_vibrato(&dict_type).expect("Failed to initialize tokenizer"));
    let frequency_manager = Arc::new(frequency_dict::process_frequency_dictionaries().expect("Failed to load Frequency Manager"));
 
    let language_tools = LanguageTools {
        tokenizer: tokenizer,
        frequency_manager: frequency_manager,
    };

    let (terms, sentences) = match process_source_file(
        &source_file,
        model_mapping.clone(),
        &language_tools,
    ).await {
        Ok(result) => result,
        Err(_) => {
            println!("Failed to process source file");
            (Vec::new(), Vec::new())
        }
    };

    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native("Yomine App", native_options, Box::new(|cc| Ok(Box::new(YomineApp::new(cc, terms, sentences, model_mapping, language_tools)))));
}