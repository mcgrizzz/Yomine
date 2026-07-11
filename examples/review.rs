//! Random-sentence pipeline review: run real sentences through the full
//! pipeline (installed frequency dictionaries, deinflection, phrase promotion,
//! N-best rescue) and print the results for eyeballing. Findings become
//! fixtures in tests/fixtures/segmentation/.
//!
//! Fetched sentences accumulate in tests/fixtures/review_corpus.txt (deduped),
//! so the reviewable corpus grows run over run; re-review the whole corpus
//! with `--file tests/fixtures/review_corpus.txt`.
//!
//! Usage:
//!   cargo run --example review              # 20 new Tatoeba sentences
//!   cargo run --example review -- 50        # 50 of them
//!   cargo run --example review -- 30 --file path/to/subs.srt   # sample a file

use rand::seq::SliceRandom;
use yomine::{
    core::models::Sentence,
    dictionary::{
        frequency_manager::process_frequency_dictionaries,
        token_dictionary::DictType,
    },
    segmentation::tokenizer::{
        extract_words,
        init_vibrato,
    },
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let count: usize = args.first().and_then(|a| a.parse().ok()).unwrap_or(20);
    let file =
        args.iter().position(|a| a == "--file").and_then(|i| args.get(i + 1)).map(String::from);

    let texts = match &file {
        Some(path) => sample_file(path, count),
        None => save_to_corpus(fetch_tatoeba(count)),
    };
    if texts.is_empty() {
        eprintln!("no sentences to review");
        std::process::exit(1);
    }

    let manager = process_frequency_dictionaries(None).expect("frequency dictionaries");
    if manager.get_enabled_dictionaries().is_empty() {
        eprintln!("!!! no frequency dictionaries installed — deinflection/rescue/phrases inactive");
    }
    let tokenizer = init_vibrato(&DictType::Unidic, None).expect("UniDic tokenizer unavailable");

    for (n, text) in texts.iter().enumerate() {
        let mut sentences = vec![Sentence {
            id: 0,
            source_id: 0,
            text: text.clone(),
            segments: Vec::new(),
            timestamp: None,
            comprehension: 0.0,
        }];
        let terms = extract_words(tokenizer.new_worker(), &mut sentences, &manager);

        println!("\n{:>3}. {}", n + 1, text);
        let furigana: String = sentences[0]
            .segments
            .iter()
            .map(|(reading, _, start, end)| {
                let surface = &text[*start..*end];
                if reading.is_empty() || reading == surface {
                    surface.to_string()
                } else {
                    format!("{}({})", surface, reading)
                }
            })
            .collect::<Vec<_>>()
            .join("|");
        println!("     {}", furigana);
        for t in &terms {
            let freq = match t.frequencies.get("HARMONIC") {
                Some(&f) if f != u32::MAX => format!("#{}", f),
                _ => "#—".to_string(),
            };
            println!(
                "     ・{:<12} → {} ({})  [{}] {}",
                t.full_segment,
                t.lemma_form,
                t.lemma_reading,
                t.part_of_speech.as_key(),
                freq
            );
        }
    }
    println!();
}

fn fetch_tatoeba(count: usize) -> Vec<String> {
    let url = format!(
        "https://tatoeba.org/en/api_v0/search?from=jpn&sort=random&limit={}",
        count.min(100)
    );
    let body: serde_json::Value = reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", "yomine-review")
        .send()
        .and_then(|r| r.json())
        .expect("tatoeba fetch failed");
    let mut texts: Vec<String> = body["results"]
        .as_array()
        .map(|results| {
            results.iter().filter_map(|r| r["text"].as_str().map(String::from)).collect()
        })
        .unwrap_or_default();
    // The API ignores small limits and returns a full page.
    texts.truncate(count);
    texts
}

/// Append fetched sentences to the corpus (skipping ones already in it) and
/// return only the new ones for review.
fn save_to_corpus(fetched: Vec<String>) -> Vec<String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/review_corpus.txt");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let known: std::collections::HashSet<&str> = existing.lines().collect();

    let mut new_texts: Vec<String> = Vec::new();
    for text in fetched {
        if !known.contains(text.as_str()) && !new_texts.contains(&text) {
            new_texts.push(text);
        }
    }
    let known_count = known.len();
    if !new_texts.is_empty() {
        let mut out = existing;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        for text in &new_texts {
            out.push_str(text);
            out.push('\n');
        }
        std::fs::write(&path, &out).expect("write review corpus");
    }
    println!(
        "{} new sentence{} (corpus now {})",
        new_texts.len(),
        if new_texts.len() == 1 { "" } else { "s" },
        known_count + new_texts.len()
    );
    new_texts
}

fn sample_file(path: &str, count: usize) -> Vec<String> {
    let content = std::fs::read_to_string(path).expect("read --file");
    let mut lines: Vec<String> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty()
                && !l.contains("-->")
                && !l.chars().all(|c| c.is_ascii())
                && l.chars().any(|c| {
                    matches!(c, '\u{3040}'..='\u{30FF}' | '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}')
                })
        })
        .map(String::from)
        .collect();
    lines.dedup();
    lines.shuffle(&mut rand::rng());
    lines.truncate(count);
    lines
}
