//! Expected-vs-actual segmentation regression suite (mission objective #3).
//!
//! Cases live in `tests/fixtures/segmentation/*.toml` — one file per theme, each
//! optionally carrying its own synthetic frequency dictionary so the
//! frequency-dependent stages (deinflection ranking, phrase promotion) are
//! deterministic and independent of whatever dictionaries a machine has
//! installed. Fixture schema:
//!
//! ```toml
//! # Synthetic dictionary for this file (optional). Lower rank = more frequent.
//! frequencies = [{ term = "行く", reading = "いく", rank = 100 }]
//!
//! [[case]]
//! name = "what this pins"
//! text = "the sentence"
//! # Optional: the FULL ordered segment list (surface required; reading/pos
//! # asserted only when present; pos uses POS::as_key names).
//! segments = [{ surface = "8月", reading = "はちがつ", pos = "Counter" }]
//! # Optional: per-term expectations, matched by surface (full_segment or the
//! # main word's surface_form). Only the fields you write are asserted.
//! terms = [{ surface = "食べた", lemma = "食べる", pos = "Verb" }]
//! # Optional: surfaces/lemmas that must NOT appear as terms.
//! absent = ["がつ"]
//! ```
//!
//! Runs the REAL UniDic tokenizer (auto-downloads on first run, or set
//! UNIDIC_PATH). If it can't load, the suite skips with a loud warning — set
//! `YOMINE_REQUIRE_UNIDIC=1` (CI) to turn that into a failure. All failing
//! cases are reported in one run. Interactive companion:
//! `cargo run --example segment -- "<sentence>"`.

use std::{
    fmt::Write as _,
    fs,
    path::PathBuf,
    sync::OnceLock,
};

use serde::Deserialize;
use yomine::{
    core::models::Sentence,
    dictionary::{
        frequency_dict::FrequencyDictionary,
        frequency_manager::FrequencyManager,
        token_dictionary::DictType,
        JsonFrequency,
        JsonFrequencyData,
        TermMetaBankV3,
    },
    segmentation::tokenizer::{
        extract_words,
        init_vibrato,
    },
    vibrato::Tokenizer,
};

// ---------------------------------------------------------------------------
// Fixture schema
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct FixtureFile {
    #[serde(default)]
    frequencies: Vec<FreqEntry>,
    #[serde(default, rename = "case")]
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct FreqEntry {
    term: String,
    reading: String,
    rank: u32,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text: String,
    #[serde(default)]
    segments: Option<Vec<SegmentExpectation>>,
    #[serde(default)]
    terms: Vec<TermExpectation>,
    #[serde(default)]
    absent: Vec<String>,
}

#[derive(Deserialize)]
struct SegmentExpectation {
    surface: String,
    reading: Option<String>,
    pos: Option<String>,
}

#[derive(Deserialize)]
struct TermExpectation {
    surface: String,
    lemma: Option<String>,
    /// `lemma_reading`.
    reading: Option<String>,
    surface_reading: Option<String>,
    pos: Option<String>,
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// One tokenizer for the whole run (UniDic load is the expensive part).
fn tokenizer() -> Option<&'static Tokenizer> {
    static TOKENIZER: OnceLock<Option<Tokenizer>> = OnceLock::new();
    TOKENIZER
        .get_or_init(|| match init_vibrato(&DictType::Unidic, None) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("!!! UniDic tokenizer unavailable ({e}); set UNIDIC_PATH or allow the download.");
                None
            }
        })
        .as_ref()
}

/// Deterministic in-memory dictionary from the fixture's `frequencies` table.
fn build_manager(entries: &[FreqEntry]) -> FrequencyManager {
    if entries.is_empty() {
        return FrequencyManager::from_dictionaries(Vec::new());
    }
    let metas = entries
        .iter()
        .map(|e| TermMetaBankV3 {
            term: e.term.clone(),
            data_type: "freq".to_string(),
            data: Some(JsonFrequencyData::Nested {
                reading: e.reading.clone(),
                frequency: JsonFrequency::Number(e.rank),
            }),
        })
        .collect();
    FrequencyManager::from_dictionaries(vec![FrequencyDictionary::new(
        "TEST".to_string(),
        "test".to_string(),
        metas,
    )])
}

/// Run one case; returns human-readable failure lines (empty = pass).
fn run_case(tok: &Tokenizer, manager: &FrequencyManager, case: &Case) -> Vec<String> {
    let mut failures = Vec::new();
    let mut sentences = vec![Sentence {
        id: 0,
        source_id: 0,
        text: case.text.clone(),
        segments: Vec::new(),
        timestamp: None,
        comprehension: 0.0,
    }];
    let terms = extract_words(tok.new_worker(), &mut sentences, manager);

    // (surface slice, reading, pos-key) actually produced for the sentence.
    let actual_segments: Vec<(String, String, String)> = sentences[0]
        .segments
        .iter()
        .map(|(reading, pos, start, end)| {
            (case.text[*start..*end].to_string(), reading.clone(), pos.as_key().to_string())
        })
        .collect();

    if let Some(expected) = &case.segments {
        if expected.len() != actual_segments.len() {
            failures.push(format!(
                "segment count: expected {}, got {}",
                expected.len(),
                actual_segments.len()
            ));
        }
        for (i, exp) in expected.iter().enumerate() {
            let Some((surface, reading, pos)) = actual_segments.get(i) else { break };
            if &exp.surface != surface {
                failures.push(format!(
                    "segment[{i}] surface: expected {:?}, got {:?}",
                    exp.surface, surface
                ));
                continue; // reading/pos comparisons are meaningless off-surface
            }
            if let Some(r) = &exp.reading {
                if r != reading {
                    failures.push(format!(
                        "segment[{i}] {:?} reading: expected {:?}, got {:?}",
                        exp.surface, r, reading
                    ));
                }
            }
            if let Some(p) = &exp.pos {
                if p != pos {
                    failures.push(format!(
                        "segment[{i}] {:?} pos: expected {:?}, got {:?}",
                        exp.surface, p, pos
                    ));
                }
            }
        }
    }

    for exp in &case.terms {
        let Some(term) =
            terms.iter().find(|t| t.full_segment == exp.surface || t.surface_form == exp.surface)
        else {
            failures.push(format!("term {:?}: not found", exp.surface));
            continue;
        };
        if let Some(lemma) = &exp.lemma {
            if lemma != &term.lemma_form {
                failures.push(format!(
                    "term {:?} lemma: expected {:?}, got {:?}",
                    exp.surface, lemma, term.lemma_form
                ));
            }
        }
        if let Some(reading) = &exp.reading {
            if reading != &term.lemma_reading {
                failures.push(format!(
                    "term {:?} reading: expected {:?}, got {:?}",
                    exp.surface, reading, term.lemma_reading
                ));
            }
        }
        if let Some(surface_reading) = &exp.surface_reading {
            if surface_reading != &term.surface_reading {
                failures.push(format!(
                    "term {:?} surface_reading: expected {:?}, got {:?}",
                    exp.surface, surface_reading, term.surface_reading
                ));
            }
        }
        if let Some(pos) = &exp.pos {
            if pos != term.part_of_speech.as_key() {
                failures.push(format!(
                    "term {:?} pos: expected {:?}, got {:?}",
                    exp.surface,
                    pos,
                    term.part_of_speech.as_key()
                ));
            }
        }
    }

    for surface in &case.absent {
        if terms.iter().any(|t| {
            &t.full_segment == surface || &t.surface_form == surface || &t.lemma_form == surface
        }) {
            failures.push(format!("term {surface:?}: expected ABSENT, but was extracted"));
        }
    }

    // Context dump so a failure is debuggable straight from the test output.
    if !failures.is_empty() {
        let mut dump = String::from("  actual segments:");
        for (surface, reading, pos) in &actual_segments {
            let _ = write!(dump, " [{surface}|{reading}|{pos}]");
        }
        dump.push_str("\n  actual terms:");
        for t in &terms {
            let _ = write!(
                dump,
                " [{}|lemma {} ({})|{}]",
                t.full_segment,
                t.lemma_form,
                t.lemma_reading,
                t.part_of_speech.as_key()
            );
        }
        failures.push(dump);
    }
    failures
}

#[test]
fn segmentation_fixtures() {
    let Some(tok) = tokenizer() else {
        if std::env::var("YOMINE_REQUIRE_UNIDIC").is_ok() {
            panic!("UniDic unavailable and YOMINE_REQUIRE_UNIDIC is set");
        }
        eprintln!("!!! SKIPPING segmentation fixtures: no UniDic dictionary.");
        return;
    };

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/segmentation");
    let mut fixture_paths: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("tests/fixtures/segmentation missing")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    fixture_paths.sort();
    assert!(!fixture_paths.is_empty(), "no fixture files found in {dir:?}");

    let mut report = String::new();
    let mut case_count = 0;
    for path in &fixture_paths {
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        let fixture: FixtureFile = toml::from_str(&fs::read_to_string(path).expect("read fixture"))
            .unwrap_or_else(|e| panic!("{file_name}: bad fixture TOML: {e}"));
        let manager = build_manager(&fixture.frequencies);
        for case in &fixture.cases {
            case_count += 1;
            let failures = run_case(tok, &manager, case);
            if !failures.is_empty() {
                let _ = writeln!(report, "\n[{file_name} / {}] {:?}", case.name, case.text);
                for f in failures {
                    let _ = writeln!(report, "  - {f}");
                }
            }
        }
    }

    assert!(report.is_empty(), "segmentation regressions ({case_count} cases run):{report}");
    println!("segmentation fixtures: {case_count} cases passed");
}
