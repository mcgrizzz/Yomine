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

/// Selects where the Anki "known terms" knowledge comes from when filtering.
pub enum AnkiFilter {
    /// Fetch live from Anki; also refreshes the on-disk vocab cache.
    Live(HashMap<String, FieldMapping>),
    /// Use the on-disk vocab snapshot (offline, fast).
    Cached,
    /// Partition by an explicit set of known lemma forms (ignore-list refresh).
    KnownLemmas(HashSet<String>),
}
use crate::{
    anki::{
        comprehensibility::calculate_sentence_comprehension,
        AnkiState,
        FieldMapping,
    },
    core::{
        LanguageTools,
        Sentence,
        SourceFile,
        Term,
    },
    parser,
    segmentation::tokenizer::extract_words,
};

pub async fn process_source_file(
    source_file: &SourceFile,
    language_tools: &LanguageTools,
) -> Result<(Vec<Term>, FilterResult, Vec<Sentence>, f32), YomineError> {
    // Parse the source file
    let sentences =
        parser::read(source_file).map_err(|e| YomineError::FailedToLoadFile(e.to_string()))?;
    println!("Parsed {} sentences", sentences.len());

    process_sentences(sentences, language_tools).await
}

/// The shared tail of file processing: tokenize/segment `sentences`, dedupe
/// terms, apply the ignore + cached-Anki filters, and compute comprehension.
/// Split from `process_source_file` so non-file sources (the asbplayer subtitle
/// importer, issue #105) run the identical pipeline.
pub async fn process_sentences(
    mut sentences: Vec<Sentence>,
    language_tools: &LanguageTools,
) -> Result<(Vec<Term>, FilterResult, Vec<Sentence>, f32), YomineError> {
    let total_start = Instant::now();

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

    // Apply filters using the cached Anki snapshot for a fast, offline-safe load.
    // The GUI refreshes against live Anki in the background when connected.
    let filter_result = apply_filters(terms, language_tools, AnkiFilter::Cached).await?;

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
    anki_filter: AnkiFilter,
) -> Result<FilterResult, YomineError> {
    let (not_ignored, mut ignore_filtered) = apply_ignore_filter(base_terms, language_tools)?;

    // Set comprehension = 1.0 for all ignored terms
    for term in &mut ignore_filtered {
        term.comprehension = 1.0;
    }

    // Apply Anki filtering
    let (unknown_terms, anki_filtered): (Vec<Term>, Vec<Term>) = match anki_filter {
        AnkiFilter::KnownLemmas(known) => {
            not_ignored.into_iter().partition(|t| !known.contains(&t.lemma_form))
        }
        AnkiFilter::Cached => {
            match AnkiState::from_cache(
                language_tools.frequency_manager.clone(),
                language_tools.known_interval,
            ) {
                Some(state) => state.filter_existing_terms(not_ignored),
                None => (not_ignored, Vec::new()),
            }
        }
        AnkiFilter::Live(model_mapping) => {
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
        }
    };

    Ok(FilterResult { terms: unknown_terms, anki_filtered, ignore_filtered })
}
