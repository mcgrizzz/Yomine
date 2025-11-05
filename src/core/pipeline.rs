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
    pub terms: Vec<Term>,           // Unknown terms for mining table
    pub anki_filtered: Vec<Term>,   // Terms matched and known by Anki
    pub ignore_filtered: Vec<Term>, // Terms filtered by ignore list
}
use crate::{
    anki::{
        comprehensibility::calculate_sentence_comprehension,
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
) -> Result<(Vec<Term>, FilterResult, Vec<Sentence>, f32), YomineError> {
    let total_start = Instant::now();

    // Parse the source file
    let mut sentences =
        parser::read(source_file).map_err(|e| YomineError::FailedToLoadFile(e.to_string()))?;
    println!("Parsed {} sentences", sentences.len());

    // Extract and deduplicate terms
    let mut terms = extract_words(
        language_tools.tokenizer.new_worker(),
        &mut sentences,
        &language_tools.frequency_manager,
    );

    terms.sort_by(|a, b| {
        a.lemma_form.cmp(&b.lemma_form).then_with(|| a.lemma_reading.cmp(&b.lemma_reading))
    });
    terms.dedup_by(|a, b| {
        a.lemma_form == b.lemma_form
            && a.lemma_reading.to_hiragana() == b.lemma_reading.to_hiragana()
    });
    println!("Extracted {} unique terms", terms.len());

    // Apply filters - comprehension calculated here
    let filter_result = apply_filters(terms, language_tools, Some(model_mapping), None).await?;

    // Reconstruct base_terms from all three sets, gathering comprehension metrics from each
    let mut base_terms = Vec::new();
    base_terms.extend(filter_result.terms.iter().cloned());
    base_terms.extend(filter_result.anki_filtered.iter().cloned());
    base_terms.extend(filter_result.ignore_filtered.iter().cloned());

    for sentence in &mut sentences {
        calculate_sentence_comprehension(sentence, &base_terms);
    }

    let file_comprehension = if !sentences.is_empty() {
        sentences.iter().map(|s| s.comprehension).sum::<f32>() / sentences.len() as f32
    } else {
        0.0
    };

    println!("Overall comprehension: {:.1}%", file_comprehension * 100.0);
    println!("Processing completed ({:.1}s)", total_start.elapsed().as_secs_f32());

    Ok((base_terms, filter_result, sentences, file_comprehension))
}

fn apply_ignore_filter(
    base_terms: Vec<Term>,
    language_tools: &LanguageTools,
) -> Result<(Vec<Term>, Vec<Term>), YomineError> {
    let ignore_list = language_tools
        .ignore_list
        .lock()
        .map_err(|_| YomineError::Custom("Failed to lock ignore list".to_string()))?;

    let (remaining, ignored): (Vec<Term>, Vec<Term>) =
        base_terms.into_iter().partition(|t| !ignore_list.contains(&t.lemma_form));

    Ok((remaining, ignored))
}

pub async fn apply_filters(
    base_terms: Vec<Term>,
    language_tools: &LanguageTools,
    model_mapping: Option<HashMap<String, FieldMapping>>,
    cached_anki_terms: Option<&HashSet<String>>,
) -> Result<FilterResult, YomineError> {
    let (not_ignored, mut ignore_filtered) = apply_ignore_filter(base_terms, language_tools)?;

    // Set comprehension = 1.0 for all ignored terms
    for term in &mut ignore_filtered {
        term.comprehension = 1.0;
    }

    // Apply Anki filtering
    let (unknown_terms, anki_filtered) = if let Some(cached) = cached_anki_terms {
        let (unknown, known): (Vec<Term>, Vec<Term>) =
            not_ignored.into_iter().partition(|t| !cached.contains(&t.lemma_form));
        (unknown, known)
    } else {
        let model_mapping = model_mapping.ok_or_else(|| {
            YomineError::Custom(
                "model_mapping required when not using cached Anki terms".to_string(),
            )
        })?;

        match AnkiState::new(
            model_mapping,
            language_tools.frequency_manager.clone(),
            language_tools.known_interval,
        )
        .await
        {
            Ok(state) => {
                let filter_anki_start = Instant::now();
                let (unknown, known) = state.filter_existing_terms(not_ignored);
                println!(
                    "Anki filtering completed ({:.1}s)",
                    filter_anki_start.elapsed().as_secs_f32()
                );
                (unknown, known)
            }
            Err(e) => {
                eprintln!("Failed to initialize AnkiState: {}", e);
                (not_ignored, Vec::new())
            }
        }
    };

    Ok(FilterResult { terms: unknown_terms, anki_filtered, ignore_filtered })
}
