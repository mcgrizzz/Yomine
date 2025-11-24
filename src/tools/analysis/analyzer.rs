use std::{
    collections::HashMap,
    fs::File,
    io::{
        BufWriter,
        Write,
    },
    path::{
        Path,
        PathBuf,
    },
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
    },
};

use wana_kana::IsJapaneseStr;
use zip::{
    write::SimpleFileOptions,
    ZipWriter,
};

use crate::{
    core::{
        SourceFile,
        Term,
        YomineError,
    },
    dictionary::{
        DictionaryIndex,
        JsonFrequency,
        JsonFrequencyData,
    },
    gui::LanguageTools,
    parser,
    segmentation::tokenizer::{
        batch_deinflect_terms,
        extract_words_for_frequency,
    },
};

#[derive(Debug, Clone)]
pub struct FrequencyAnalysisResult {
    pub frequencies: HashMap<(String, Option<String>), u32>, // (lemma_form, lemma_reading) -> count
    pub total_terms: usize,
    pub unique_terms: usize,
    pub skipped_files: Vec<(String, String)>,
}

fn should_exclude_token(lemma_form: &str) -> bool {
    if lemma_form.trim().is_empty() {
        return true;
    }

    if !lemma_form.is_japanese() {
        return true;
    }

    const EXCLUDED_TOKENS: &[&str] = &[
        "。", "｡", "、", "！", "？", "‼", "⁉", // Japanese punctuation
        "・", "･", "…", "～", "〜", "ー", // Japanese symbols
        "「", "」", "『", "』", "【", "】", // Japanese brackets
        "（", "）", "｟", "｠", "《", "》", // More brackets
        "(", ")", "[", "]", "{", "}", // ASCII brackets
        ",", ".", "!", "?", ";", ":", // ASCII punctuation
        "\"", "'", "`", "´", "“", "”", // Quotes
        "-", "–", "—", "―", "_", "＿", // Dashes and underscores
        "/", "\\", "|", "｜", // Slashes and pipes
        "*", "＊", "※", "☆", "★", // Stars and symbols
        "♪", "♬", "♫", "♩", // Music symbols
        "→", "←", "↑", "↓", "➡", "⇒", // Arrows
        "○", "●", "◯", "◎", "□", "■", // Shapes
        "◆", "◇", "△", "▲", "▽", "▼",  // More shapes
        "　", // Full-width space
        "＃", "#", "&", "＆", "@", "＠", // Symbols
        "+", "＋", "=", "＝", "<", ">", // Math symbols
        "＜", "＞", "≪", "≫", // More comparison symbols
        "%", "％", "$", "￥", "¥", // Currency/percent
        "°", "℃", "℉", // Degrees
        "〒", "§", // Special symbols
    ];

    EXCLUDED_TOKENS.contains(&lemma_form)
}

pub fn analyze_files(
    file_paths: Vec<PathBuf>,
    language_tools: &LanguageTools,
    progress_callback: Option<Box<dyn Fn(usize, String, u64) + Send + Sync>>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<FrequencyAnalysisResult, YomineError> {
    let total_files = file_paths.len();
    let mut global_surface_frequencies: HashMap<(String, String), u32> = HashMap::new();
    let mut unique_terms: HashMap<(String, String), Term> = HashMap::new();
    let mut total_terms = 0;
    let mut skipped_files: Vec<(String, String)> = Vec::new();

    for (idx, file_path) in file_paths.iter().enumerate() {
        // Check for cancellation
        if let Some(ref flag) = cancel_flag {
            if flag.load(Ordering::Relaxed) {
                return Err(YomineError::Custom("Analysis cancelled by user".to_string()));
            }
        }

        let file_path_str = file_path.to_string_lossy().to_string();
        let file_name =
            file_path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown").to_string();

        let file_size = std::fs::metadata(file_path).ok().map(|m| m.len()).unwrap_or(0);

        if let Some(ref callback) = progress_callback {
            callback(idx + 1, file_name.clone(), file_size);
        }

        let source_file = SourceFile {
            id: idx as u32,
            source: None,
            file_type: crate::core::models::SourceFileType::from_extension(&file_path_str),
            title: file_name.clone(),
            creator: None,
            original_file: file_path_str,
        };

        let mut sentences = match parser::read(&source_file) {
            Ok(s) => s,
            Err(e) => {
                // Track skipped files and continue processing
                skipped_files.push((file_name.clone(), e.to_string()));
                if let Some(ref callback) = progress_callback {
                    callback(idx + 1, format!("⚠ Skipped {}: {}", file_name, e), file_size);
                }
                continue;
            }
        };

        if sentences.is_empty() {
            continue;
        }

        // Extract terms from this file
        let file_terms = extract_words_for_frequency(
            language_tools.tokenizer.new_worker(),
            &mut sentences,
            &language_tools.frequency_manager,
            None,
        );

        // Count unique terms within this file only
        let mut file_surface_frequencies: HashMap<(String, String), u32> = HashMap::new();

        for term in file_terms {
            // Skip excluded tokens (punctuation, symbols, etc.)
            if should_exclude_token(&term.surface_form) {
                continue;
            }

            // Skip onomatopoeia and pure numbers
            use crate::segmentation::word::POS;
            if matches!(term.part_of_speech, POS::Onomatopoeia | POS::Number) {
                continue;
            }

            total_terms += 1;

            // Count the main word (surface_form)
            let main_key = (term.surface_form.clone(), term.surface_reading.clone());
            *file_surface_frequencies.entry(main_key.clone()).or_insert(0) += 1;
            unique_terms.entry(main_key).or_insert(term.clone());

            // Also count the full compound if it's different from the main word
            if !term.full_segment.is_empty() && term.full_segment != term.surface_form {
                let full_key = (term.full_segment.clone(), term.full_segment_reading.clone());
                *file_surface_frequencies.entry(full_key.clone()).or_insert(0) += 1;

                // Create a term for the full segment (preserve POS from main word)
                let mut full_term = term.clone();
                full_term.surface_form = term.full_segment.clone();
                full_term.surface_reading = term.full_segment_reading.clone();
                unique_terms.entry(full_key).or_insert(full_term);
            }
        }

        // Merge into global counts
        for (key, count) in file_surface_frequencies {
            *global_surface_frequencies.entry(key).or_insert(0) += count;
        }
    }

    if global_surface_frequencies.is_empty() {
        return Err(YomineError::Custom("No terms found in any of the files".to_string()));
    }

    // Check for cancellation before deinflection phase
    if let Some(ref flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            return Err(YomineError::Custom("Analysis cancelled by user".to_string()));
        }
    }

    if let Some(ref callback) = progress_callback {
        callback(
            total_files,
            format!("Deinflecting {} unique surface forms...", unique_terms.len()),
            0,
        );
    }

    // Batch deinflect
    let terms_vec: Vec<Term> = unique_terms.into_values().collect();
    let deinflection_map = batch_deinflect_terms(&terms_vec, &language_tools.frequency_manager);

    if let Some(ref callback) = progress_callback {
        callback(total_files, "Merging counts by deinflected forms...".to_string(), 0);
    }

    // Merge surface form counts into deinflected forms
    let mut frequencies: HashMap<(String, Option<String>), u32> = HashMap::new();

    for ((surface_form, surface_reading), count) in global_surface_frequencies {
        let (lemma_form, lemma_reading) = deinflection_map
            .get(&(surface_form.clone(), surface_reading.clone()))
            .cloned()
            .unwrap_or((surface_form, surface_reading));

        let reading = if lemma_form.as_str().is_kana() { None } else { Some(lemma_reading) };

        let key = (lemma_form, reading);
        *frequencies.entry(key).or_insert(0) += count;
    }

    let unique_terms_count = frequencies.len();

    Ok(FrequencyAnalysisResult {
        frequencies,
        total_terms,
        unique_terms: unique_terms_count,
        skipped_files,
    })
}

fn calculate_ranks(counts: &[u32]) -> Vec<u32> {
    let mut ranks = Vec::with_capacity(counts.len());
    let mut current_rank = 1u32;
    let mut prev_count: Option<u32> = None;

    for (idx, &count) in counts.iter().enumerate() {
        let rank_value = if prev_count.is_some() && prev_count.unwrap() == count {
            current_rank // Same count = same rank
        } else {
            current_rank = (idx + 1) as u32; // New count = new rank based on position
            current_rank
        };
        prev_count = Some(count);
        ranks.push(rank_value);
    }

    ranks
}

pub fn export_yomitan_zip(
    result: &FrequencyAnalysisResult,
    dict_name: &str,
    author: Option<&str>,
    url: Option<&str>,
    description: Option<&str>,
    output_path: &Path,
    pretty: bool,
    exclude_hapax: bool,
    revision_prefix: Option<&str>,
) -> Result<(), YomineError> {
    let zip_path = output_path.join(format!("{}.zip", dict_name));
    let file = File::create(&zip_path)
        .map_err(|e| YomineError::Custom(format!("Failed to create ZIP file: {}", e)))?;

    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let final_description = match description {
        Some(desc) if !desc.is_empty() => {
            Some(format!("{}\n\nGenerated in Yomine v{}", desc, VERSION))
        }
        _ => Some(format!("Generated in Yomine v{}", VERSION)),
    };

    let date_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let revision = match revision_prefix {
        Some(prefix) if !prefix.is_empty() => {
            format!("{}.frequency.{}", prefix, date_str)
        }
        _ => date_str,
    };

    let index = DictionaryIndex {
        title: dict_name.to_string(),
        revision,
        format: Some(3),
        version: None,
        author: author.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        url: url.map(|s| s.to_string()).filter(|s| !s.is_empty()),
        description: final_description,
        frequency_mode: Some("rank-based".to_string()),
    };

    let index_json =
        if pretty { serde_json::to_string_pretty(&index) } else { serde_json::to_string(&index) }
            .map_err(|e| YomineError::Custom(format!("Failed to serialize index: {}", e)))?;

    zip.start_file("index.json", options)
        .map_err(|e| YomineError::Custom(format!("Failed to create index.json: {}", e)))?;
    zip.write_all(index_json.as_bytes())
        .map_err(|e| YomineError::Custom(format!("Failed to write index.json: {}", e)))?;

    let mut lemma_counts: HashMap<String, u32> = HashMap::new();
    let mut lemma_readings: HashMap<String, Vec<Option<String>>> = HashMap::new();

    for ((lemma_form, reading), count) in &result.frequencies {
        *lemma_counts.entry(lemma_form.clone()).or_insert(0) += count;
        lemma_readings.entry(lemma_form.clone()).or_default().push(reading.clone());
    }

    if exclude_hapax {
        lemma_counts.retain(|_, count| *count > 1);
    }

    let mut sorted_lemmas: Vec<_> = lemma_counts.into_iter().collect();
    sorted_lemmas.sort_by(|a, b| b.1.cmp(&a.1));

    let counts: Vec<u32> = sorted_lemmas.iter().map(|(_, count)| *count).collect();
    let ranks = calculate_ranks(&counts);

    let mut entries: Vec<(String, String, JsonFrequencyData)> = Vec::new();

    for ((lemma_form, _count), rank_value) in sorted_lemmas.iter().zip(ranks.iter()) {
        if let Some(readings) = lemma_readings.get(lemma_form) {
            for reading in readings {
                let data = match reading {
                    Some(r) => JsonFrequencyData::Nested {
                        reading: r.clone(),
                        frequency: JsonFrequency::Number(*rank_value),
                    },
                    None => JsonFrequencyData::Simple(JsonFrequency::Number(*rank_value)),
                };
                entries.push((lemma_form.clone(), "freq".to_string(), data));
            }
        }
    }

    let term_meta_json = if pretty {
        serde_json::to_string_pretty(&entries)
    } else {
        serde_json::to_string(&entries)
    }
    .map_err(|e| YomineError::Custom(format!("Failed to serialize term meta: {}", e)))?;

    zip.start_file("term_meta_bank_1.json", options).map_err(|e| {
        YomineError::Custom(format!("Failed to create term_meta_bank_1.json: {}", e))
    })?;
    zip.write_all(term_meta_json.as_bytes()).map_err(|e| {
        YomineError::Custom(format!("Failed to write term_meta_bank_1.json: {}", e))
    })?;

    zip.finish().map_err(|e| YomineError::Custom(format!("Failed to finalize ZIP: {}", e)))?;

    Ok(())
}

/// Export frequency data as a CSV file
pub fn export_csv(
    result: &FrequencyAnalysisResult,
    output_path: &Path,
    csv_name: &str,
    exclude_hapax: bool,
) -> Result<(), YomineError> {
    let csv_path = output_path.join(format!("{}.csv", csv_name));

    let mut entries: Vec<_> =
        result.frequencies.iter().filter(|(_, count)| !exclude_hapax || **count > 1).collect();

    entries.sort_by(|a, b| b.1.cmp(a.1));

    let counts: Vec<u32> = entries.iter().map(|(_, count)| **count).collect();
    let ranks = calculate_ranks(&counts);

    let file = File::create(&csv_path)
        .map_err(|e| YomineError::Custom(format!("Failed to create CSV file: {}", e)))?;
    let mut writer = BufWriter::new(file);

    writeln!(writer, "Rank,Term,Reading,Frequency")
        .map_err(|e| YomineError::Custom(format!("Failed to write CSV header: {}", e)))?;

    for (((lemma_form, lemma_reading), count), rank_value) in entries.iter().zip(ranks.iter()) {
        let reading_str = lemma_reading.as_deref().unwrap_or("");
        writeln!(writer, "{},\"{}\",\"{}\",{}", rank_value, lemma_form, reading_str, count)
            .map_err(|e| YomineError::Custom(format!("Failed to write CSV row: {}", e)))?;
    }

    writer.flush().map_err(|e| YomineError::Custom(format!("Failed to flush CSV file: {}", e)))?;

    Ok(())
}
