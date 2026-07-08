use std::sync::Arc;

use crate::{
    anki::{
        comprehensibility::comp_term,
        AnkiState,
    },
    dictionary::frequency_manager::FrequencyManager,
    jlpt::{
        JlptDatabase,
        JlptLevel,
    },
};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BandStats {
    /// Fraction (0..1) of the band's words present in Anki, regardless of study state.
    pub coverage: f32,
    /// Average estimated comprehension (0..1) across the band's words, using the same
    /// per-word `comp_term` estimate as sentence/file comprehension.
    pub comprehension: f32,
    /// Number of reference words in the band.
    pub total: usize,
}

impl BandStats {
    fn averaged(coverage_sum: f32, comprehension_sum: f32, total: usize) -> Self {
        let (coverage, comprehension) = if total > 0 {
            (coverage_sum / total as f32, comprehension_sum / total as f32)
        } else {
            (0.0, 0.0)
        };
        Self { coverage, comprehension, total }
    }
}

/// Which estimate the knowledge widget shows: raw Anki presence vs the graded comprehension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum KnowledgeMode {
    #[default]
    Coverage,
    Estimate,
}

impl KnowledgeMode {
    pub fn title(self) -> &'static str {
        match self {
            Self::Coverage => "Anki Coverage",
            Self::Estimate => "Estimated Knowledge",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Coverage => Self::Estimate,
            Self::Estimate => Self::Coverage,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeSummary {
    pub jlpt: Vec<(JlptLevel, BandStats)>,
    pub frequency: Vec<(String, BandStats)>,
}

/// Frequency bands with geometrically-growing boundaries (Zipf's law: equal rank *ratios*
/// are equal steps), capped at ~20k where rank for now.
/// Each tuple is `(low_rank_inclusive, high_rank_inclusive, label)`.
const FREQUENCY_BANDS: &[(u32, u32, &str)] = &[
    (1, 1_500, "<1.5k"),
    (1_501, 3_000, "<3k"),
    (3_001, 5_000, "<5k"),
    (5_001, 10_000, "<10k"),
    (10_001, 20_000, "<20k"),
];

fn band_for_rank(rank: u32) -> Option<usize> {
    FREQUENCY_BANDS.iter().position(|(lo, hi, _)| rank >= *lo && rank <= *hi)
}

/// Global JLPT / frequency comprehension, computed offline from the on-disk Anki vocab
/// cache. Callers recompute after a live Anki refresh rewrites the cache.
pub fn compute_knowledge_summary(
    frequency_manager: Arc<FrequencyManager>,
    known_interval: u32,
) -> KnowledgeSummary {
    let mut summary = KnowledgeSummary::default();

    let Some(anki) = AnkiState::from_cache(frequency_manager.clone(), known_interval) else {
        return summary; // No Anki vocab cache yet; nothing to report against.
    };

    {
        let db = JlptDatabase::load();
        for level in JlptLevel::ALL {
            let mut coverage_sum = 0.0;
            let mut comprehension_sum = 0.0;
            let mut total = 0;
            for entry in db.entries_for_level(level) {
                // Kana-only entries have an empty kanji form; match on the kana instead.
                let term = if entry.kanji.is_empty() { &entry.kana } else { &entry.kanji };
                let (in_anki, comp) = anki.word_stats(term, &entry.kana, &entry.pos);
                coverage_sum += in_anki as u8 as f32;
                comprehension_sum += comp;
                total += 1;
            }
            summary.jlpt.push((level, BandStats::averaged(coverage_sum, comprehension_sum, total)));
        }
    }

    {
        // Bucket each known Anki word by its frequency rank — no need to scan the whole
        // dictionary. A band's denominator is its rank width (number of slots), so unknown
        // words simply contribute nothing while known words add their comprehension.
        let max_rank = frequency_manager
            .get_enabled_dictionaries()
            .iter()
            .map(|dict| dict.terms.len() as u32)
            .max()
            .unwrap_or(0);

        let mut coverage_sums = vec![0.0_f32; FREQUENCY_BANDS.len()];
        let mut comprehension_sums = vec![0.0_f32; FREQUENCY_BANDS.len()];
        for vocab in anki.vocab() {
            if let Some(rank) =
                frequency_manager.get_harmonic_frequency_for_pair(&vocab.term, &vocab.reading)
            {
                if let Some(band) = band_for_rank(rank) {
                    coverage_sums[band] += 1.0;
                    comprehension_sums[band] += comp_term(vocab.interval, known_interval);
                }
            }
        }

        for (idx, (lo, hi, label)) in FREQUENCY_BANDS.iter().enumerate() {
            let hi = (*hi).min(max_rank);
            if hi < *lo {
                continue; // Band lies beyond the dictionary's size.
            }
            let size = (hi - *lo + 1) as usize;
            let coverage = (coverage_sums[idx] / size as f32).clamp(0.0, 1.0);
            let comprehension = (comprehension_sums[idx] / size as f32).clamp(0.0, 1.0);
            summary
                .frequency
                .push((label.to_string(), BandStats { coverage, comprehension, total: size }));
        }
    }

    summary
}
