use std::{
    collections::{
        HashMap,
        HashSet,
    },
    time::Instant,
};

use wana_kana::ConvertJapanese;

use super::YomineError;

#[derive(Debug, Clone)]
pub struct FilterResult {
    pub terms: Vec<Term>,
    pub anki_filtered: HashSet<String>,
}
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
) -> Result<(Vec<Term>, FilterResult, Vec<Sentence>), YomineError> {
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

    let filter_result = apply_filters(terms, language_tools, Some(model_mapping), None).await?;

    // Total time
    let total_duration = total_start.elapsed();
    println!("Total processing time: {:?}", total_duration);

    Ok((base_terms, filter_result, sentences))
}

fn apply_ignore_filter(
    base_terms: Vec<Term>,
    language_tools: &LanguageTools,
) -> Result<Vec<Term>, YomineError> {
    let ignore_list = language_tools
        .ignore_list
        .lock()
        .map_err(|_| YomineError::Custom("Failed to lock ignore list".to_string()))?;

    Ok(base_terms.into_iter().filter(|t| !ignore_list.contains(&t.lemma_form)).collect())
}

pub async fn apply_filters(
    base_terms: Vec<Term>,
    language_tools: &LanguageTools,
    model_mapping: Option<HashMap<String, FieldMapping>>,
    cached_anki_terms: Option<&HashSet<String>>,
) -> Result<FilterResult, YomineError> {
    let mut terms = apply_ignore_filter(base_terms, language_tools)?;

    let anki_filtered = if let Some(cached) = cached_anki_terms {
        terms = terms.into_iter().filter(|t| !cached.contains(&t.lemma_form)).collect();
        cached.clone()
    } else {
        let model_mapping = model_mapping.ok_or_else(|| {
            YomineError::Custom(
                "model_mapping required when not using cached Anki terms".to_string(),
            )
        })?;

        let mut anki_filtered = HashSet::new();
        match AnkiState::new(model_mapping, language_tools.frequency_manager.clone()).await {
            Ok(state) => {
                let filter_anki_start = Instant::now();

                // Track which terms are filtered by Anki
                let before_anki: HashSet<String> =
                    terms.iter().map(|t| t.lemma_form.clone()).collect();
                terms = state.filter_existing_terms(terms);
                let after_anki: HashSet<String> =
                    terms.iter().map(|t| t.lemma_form.clone()).collect();

                // Compute what was filtered out by Anki
                anki_filtered = before_anki.difference(&after_anki).cloned().collect();

                let filter_anki_duration = filter_anki_start.elapsed();
                println!("Filtering terms against Anki took: {:?}", filter_anki_duration);
                println!("Filtered: {}", terms.len());
            }
            Err(e) => {
                eprintln!("Failed to initialize AnkiState: {}", e);
            }
        }
        anki_filtered
    };

    Ok(FilterResult { terms, anki_filtered })
}
