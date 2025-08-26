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
) -> Result<(Vec<Term>, Vec<Term>, Vec<Sentence>), YomineError> {
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

    // Deduplicate terms early to form a refresh baseline
    terms.sort_by(|a, b| {
        a.lemma_form.cmp(&b.lemma_form).then_with(|| a.lemma_reading.cmp(&b.lemma_reading))
    });
    terms.dedup_by(|a, b| {
        a.lemma_form == b.lemma_form
            && a.lemma_reading.to_hiragana() == b.lemma_reading.to_hiragana()
    });
    println!("Deduplicated: {}", terms.len());

    // Keep a baseline copy for future refresh without re-tokenizing
    let base_terms = terms.clone();

    let terms = apply_filters(terms, model_mapping, language_tools).await?;

    // Total time
    let total_duration = total_start.elapsed();
    println!("Total processing time: {:?}", total_duration);

    Ok((base_terms, terms, sentences))
}

/// Reapply filtering to a baseline set of terms without re-tokenizing.
/// 1) Applies the current ignore list
/// 2) Fetches a fresh Anki state and filters again
pub async fn apply_filters(
    base_terms: Vec<Term>,
    model_mapping: std::collections::HashMap<String, FieldMapping>,
    language_tools: &LanguageTools,
) -> Result<Vec<Term>, YomineError> {
    let mut terms = {
        let ignore_list = language_tools
            .ignore_list
            .lock()
            .map_err(|_| YomineError::Custom("Failed to lock ignore list".to_string()))?;
        base_terms
            .into_iter()
            .filter(|t| !ignore_list.contains(&t.lemma_form))
            .collect::<Vec<Term>>()
    };

    match AnkiState::new(model_mapping, language_tools.frequency_manager.clone()).await {
        Ok(state) => {
            let filter_anki_start = Instant::now();
            terms = state.filter_existing_terms(terms);

            let filter_anki_duration = filter_anki_start.elapsed();
            println!("Filtering terms against Anki took: {:?}", filter_anki_duration);
            println!("Filtered: {}", terms.len());
        }
        Err(e) => {
            eprintln!("Failed to initialize AnkiState in reapply_filters: {}", e);
        }
    }

    Ok(terms)
}
