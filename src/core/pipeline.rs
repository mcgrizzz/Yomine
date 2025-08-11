use std::{
    collections::HashMap,
    time::Instant,
};

use wana_kana::ConvertJapanese;

use super::YomineError;
use crate::{
    anki::{
        AnkiState,
        FieldMapping,
    },
    core::{
        Sentence,
        SourceFile,
        Term,
    },
    gui::LanguageTools,
    parser,
    segmentation::tokenizer::extract_words,
};

pub async fn process_source_file(
    source_file: &SourceFile,
    model_mapping: HashMap<String, FieldMapping>,
    language_tools: &LanguageTools,
) -> Result<(Vec<Term>, Vec<Sentence>), YomineError> {
    // Start total timing
    let total_start = Instant::now();

    // Parse the source file
    //let parse_start = Instant::now();
    let mut sentences =
        parser::read(source_file).map_err(|e| YomineError::FailedToLoadFile(e.to_string()))?;

    //let parse_duration = parse_start.elapsed();
    //println!("Parsing source file took: {:?}", parse_duration);
    println!("Parsed {} sentences", sentences.len());

    // Extract terms
    //let extract_start = Instant::now();
    let mut terms = extract_words(
        language_tools.tokenizer.new_worker(),
        &mut sentences,
        &language_tools.frequency_manager,
    );
    //let extract_duration = extract_start.elapsed();
    //println!("Extracting terms took: {:?}", extract_duration);
    println!("Extracted {} terms", terms.len());

    // Filter ignored terms
    //let filter_ignored_start = Instant::now();
    terms = {
        let ignore_list = language_tools
            .ignore_list
            .lock()
            .map_err(|_| YomineError::Custom("Failed to lock ignore list".to_string()))?;
        terms
            .into_iter()
            .filter(|term| !ignore_list.contains(&term.lemma_form))
            .collect::<Vec<Term>>()
    };
    //let filter_ignored_duration = filter_ignored_start.elapsed();
    //println!("Filtering ignored terms took: {:?}", filter_ignored_duration);
    println!("Prefiltered: {}", terms.len());

    // Initialize Anki state
    //let anki_init_start = Instant::now();
    let anki_state =
        match AnkiState::new(model_mapping, language_tools.frequency_manager.clone()).await {
            Ok(state) => Some(state),
            Err(e) => {
                eprintln!("Failed to initialize AnkiState: {}", e);
                None
            }
        };
    //let anki_init_duration = anki_init_start.elapsed();
    //println!("Initializing Anki state took: {:?}", anki_init_duration);

    // Deduplicate terms
    //let dedup_start = Instant::now();
    terms.sort_by(|a, b| {
        a.lemma_form.cmp(&b.lemma_form).then_with(|| a.lemma_reading.cmp(&b.lemma_reading))
    });
    terms.dedup_by(|a, b| {
        a.lemma_form == b.lemma_form
            && a.lemma_reading.to_hiragana() == b.lemma_reading.to_hiragana()
    });
    //let dedup_duration = dedup_start.elapsed();
    //println!("Deduplicating terms took: {:?}", dedup_duration);
    println!("Deduplicated: {}", terms.len());

    // Filter terms against Anki (if applicable)
    if let Some(state) = anki_state {
        let filter_anki_start = Instant::now();
        terms = state.filter_existing_terms(terms);

        let filter_anki_duration = filter_anki_start.elapsed();
        println!("Filtering terms against Anki took: {:?}", filter_anki_duration);
        println!("Filtered: {}", terms.len());
    }

    // Total time
    let total_duration = total_start.elapsed();
    println!("Total processing time: {:?}", total_duration);

    Ok((terms, sentences))
}
