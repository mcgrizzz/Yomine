//! Segmentation debug dumper (mission objective #3): print every pipeline stage
//! for one sentence — raw UniDic features → parsed tokens → rule-matched words →
//! extracted terms + display segments. The interactive companion to the
//! expected-vs-actual suite in `tests/segmentation.rs`.
//!
//! Usage: cargo run --example segment -- "8月22日に行きます。"
//! Needs UniDic (auto-downloads on first run, or set UNIDIC_PATH). Runs with an
//! empty frequency manager, so deinflection keeps the UniDic lemma and no
//! phrases are promoted — pass real sentences through the app for those.

use yomine::{
    core::models::Sentence,
    dictionary::{
        frequency_manager::FrequencyManager,
        token_dictionary::DictType,
    },
    segmentation::{
        rule_matcher::parse_into_words,
        token_models::{
            RawToken,
            UnidicToken,
            VibratoToken,
        },
        tokenizer::{
            extract_words,
            init_vibrato,
        },
    },
};

fn main() {
    let text: String = std::env::args().skip(1).collect::<Vec<_>>().join(" ").trim().to_string();
    if text.is_empty() {
        eprintln!("usage: cargo run --example segment -- \"<sentence>\"");
        std::process::exit(2);
    }

    let tokenizer = init_vibrato(&DictType::Unidic, None).expect("UniDic tokenizer unavailable");

    let mut worker = tokenizer.new_worker();
    worker.reset_sentence(&text);
    worker.tokenize();

    println!("== raw vibrato features ==");
    let tokens: Vec<UnidicToken> = worker
        .token_iter()
        .map(|t| {
            println!("{:8} | {}", t.surface(), t.feature());
            let vt = VibratoToken {
                surface: t.surface().to_string(),
                features: t.feature().to_string(),
            };
            let surface = vt.surface.clone();
            let raw: RawToken = vt.into();
            (surface, raw).into()
        })
        .collect();

    println!("\n== parsed tokens ==");
    for t in &tokens {
        println!(
            "{:8} pos1={:?} pron={:?} lemma={:?} lemma_pron={:?}",
            t.surface, t.pos1, t.surface_hatsuon, t.lemma_form, t.lemma_hatsuon
        );
    }

    println!("\n== words (rule matcher) ==");
    let words = parse_into_words(tokens).expect("rule matcher failed");
    for w in &words {
        println!(
            "{:10} reading={:10} lemma={} ({}) pos={:?} main={:?}",
            w.surface_form,
            w.surface_hatsuon,
            w.lemma_form,
            w.lemma_hatsuon,
            w.part_of_speech,
            w.main_word.as_ref().map(|m| m.surface.as_str())
        );
    }

    println!("\n== terms (extract_words) ==");
    let manager = FrequencyManager::from_dictionaries(Vec::new());
    let mut sentences = vec![Sentence {
        id: 0,
        source_id: 0,
        text: text.clone(),
        segments: Vec::new(),
        timestamp: None,
        comprehension: 0.0,
    }];
    let terms = extract_words(tokenizer.new_worker(), &mut sentences, &manager);
    for t in &terms {
        println!(
            "{:10} lemma={} ({}) surface_reading={} pos={:?}",
            t.surface_form, t.lemma_form, t.lemma_reading, t.surface_reading, t.part_of_speech
        );
    }

    println!("\n== display segments ==");
    for (reading, pos, start, end) in &sentences[0].segments {
        println!("[{start:3}..{end:3}] {reading} {pos:?}");
    }
}
