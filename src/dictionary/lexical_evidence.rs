//! Reading-aware lexical evidence. Dictionary settings and Anki cards are not inputs.
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    sync::{
        Arc,
        Mutex,
        OnceLock,
    },
};

use wana_kana::IsJapaneseStr;

use crate::{
    core::utils::{
        is_kanji_char,
        normalize_japanese_text,
    },
    dictionary::{
        frequency_manager::FrequencyManager,
        token_dictionary::DictType,
    },
    segmentation::{
        lexeme_resolver::CandidateLexemeResolver,
        tokenizer::init_vibrato,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexicalFamily {
    pub spellings: Vec<String>,
    pub resolved_lexeme: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct ReadingEvidence {
    pub families: Vec<LexicalFamily>,
}
/// Dictionary interpretation is selected before consulting the user's cards.
#[derive(Debug)]
pub(crate) enum ExpressionSelection<'a> {
    Selected { family: &'a LexicalFamily },
    Ambiguous(Vec<&'a LexicalFamily>),
    Unresolved,
}

impl ReadingEvidence {
    pub(crate) fn select_expression(
        &self,
        surface: &str,
        citation: &str,
    ) -> ExpressionSelection<'_> {
        // A validated written citation identifies the expression even when the
        // source text is kana or inflected. Never choose a homophone over it.
        for form in [citation, surface] {
            if !form.is_empty() && !form.is_kana() {
                return self.family_for(form).map_or(ExpressionSelection::Unresolved, |family| {
                    ExpressionSelection::Selected { family }
                });
            }
        }
        if surface.is_empty() || citation.is_empty() {
            return ExpressionSelection::Unresolved;
        }
        let families: Vec<_> = self.written_families().collect();
        if families.len() == 1 {
            return ExpressionSelection::Selected { family: families[0] };
        }
        if families.is_empty() {
            ExpressionSelection::Unresolved
        } else {
            ExpressionSelection::Ambiguous(families)
        }
    }

    pub fn family_for(&self, spelling: &str) -> Option<&LexicalFamily> {
        let spelling = normalize_japanese_text(spelling);
        self.families
            .iter()
            .find(|family| family.spellings.iter().any(|s| normalize_japanese_text(s) == spelling))
    }
    pub fn written_families(&self) -> impl Iterator<Item = &LexicalFamily> {
        self.families.iter().filter(|f| f.spellings.iter().any(|s| !s.as_str().is_kana()))
    }
    pub fn phrase_family(&self, surface: &str) -> Option<LexicalFamily> {
        if surface.is_empty() || !surface.is_kana() {
            return None;
        }
        let mut families = self.written_families();
        let family = families.next()?;
        (families.next().is_none()
            && family.spellings.iter().any(|s| is_kana_spelling_of(surface, s)))
        .then(|| family.clone())
    }
}

#[derive(Debug, Default)]
pub struct LexicalEvidence {
    cache: Mutex<HashMap<String, Arc<ReadingEvidence>>>,
    resolver: OnceLock<Option<CandidateLexemeResolver>>,
}
impl LexicalEvidence {
    pub fn for_reading(&self, manager: &FrequencyManager, reading: &str) -> Arc<ReadingEvidence> {
        let reading = normalize_japanese_text(reading);
        if let Some(found) = self.cache.lock().expect("lexical cache poisoned").get(&reading) {
            return found.clone();
        }
        let candidates = manager.terms_with_reading_from_all_dictionaries(&reading);
        let families = if candidates.is_empty() {
            Vec::new()
        } else {
            let resolver =
                self.resolver.get_or_init(|| shared_tokenizer().map(CandidateLexemeResolver::new));
            build_families(&candidates, |spelling| {
                resolver.as_ref().and_then(|r| r.resolve(spelling, &reading))
            })
        };
        let result = Arc::new(ReadingEvidence { families });
        self.cache.lock().expect("lexical cache poisoned").insert(reading, result.clone());
        result
    }
}

fn build_families(
    candidates: &[&str],
    resolve: impl Fn(&str) -> Option<String>,
) -> Vec<LexicalFamily> {
    let mut output: Vec<LexicalFamily> = Vec::new();
    for group in variant_families(candidates) {
        let answers: Vec<_> = group.iter().map(|s| resolve(s)).collect();
        let distinct: HashSet<_> = answers.iter().flatten().collect();
        if distinct.len() > 1 {
            output.extend(group.into_iter().zip(answers).map(|(s, resolved_lexeme)| {
                LexicalFamily { spellings: vec![s.to_string()], resolved_lexeme }
            }));
            continue;
        }
        // Missing answers do not merge otherwise distinct families.
        let resolved_lexeme = if answers.iter().all(Option::is_some) {
            answers.first().cloned().flatten()
        } else {
            None
        };
        if let Some(existing) = output
            .iter_mut()
            .find(|f| resolved_lexeme.is_some() && f.resolved_lexeme == resolved_lexeme)
        {
            existing.spellings.extend(group.iter().map(|s| s.to_string()));
        } else {
            output.push(LexicalFamily {
                spellings: group.iter().map(|s| s.to_string()).collect(),
                resolved_lexeme,
            });
        }
    }
    for f in &mut output {
        f.spellings.sort();
        f.spellings.dedup();
    }
    output.sort_by(|a, b| a.spellings.cmp(&b.spellings));
    output
}

/// Orthographic components must have an actual spelling containing every member's
/// kanji sequence. This permits もの凄い / 物すごい through 物凄い, without allowing
/// 取りつく to bridge the conflicting kanji in 取り付く and 取り憑く.
pub fn variant_families<'a>(terms: &[&'a str]) -> Vec<Vec<&'a str>> {
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }

    let mut parent: Vec<usize> = (0..terms.len()).collect();
    for a in 0..terms.len() {
        for b in (a + 1)..terms.len() {
            if is_okurigana_variant_pair(terms[a], terms[b]) {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                parent[ra] = rb;
            }
        }
    }

    let mut families: std::collections::HashMap<usize, Vec<&'a str>> =
        std::collections::HashMap::new();
    for (i, term) in terms.iter().enumerate() {
        let r = root(&mut parent, i);
        families.entry(r).or_default().push(term);
    }
    let mut result = Vec::new();
    for family in families.into_values() {
        let kanji: Vec<Vec<char>> = family
            .iter()
            .map(|term| term.chars().filter(|c| is_kanji_char(*c)).collect())
            .collect();
        let compatible = kanji.iter().any(|anchor| {
            kanji.iter().all(|member| {
                let mut remaining = anchor.iter();
                member.iter().all(|c| remaining.any(|a| a == c))
            })
        });
        if compatible {
            result.push(family);
        } else {
            // An ambiguous kana bridge does not choose either written identity.
            result.extend(family.into_iter().map(|term| vec![term]));
        }
    }
    result
}

/// The forms differ only where one side writes a segment in kanji and the
/// other in kana (話し掛ける vs 話しかける ✓; 上る vs 昇る ✗).
fn is_okurigana_variant_pair(a: &str, b: &str) -> bool {
    if a.is_kana() || b.is_kana() {
        return false;
    }
    differs_only_where_one_writes_kanji(a, b)
}

/// The kanji spelling of an all-kana form: 何となく writes なんとなく with 何 for なん.
/// Same alignment as the variant test, which excludes kana precisely because a bare kana
/// form has no identity of its own — here the kana form *is* the thing being identified.
pub fn is_kana_spelling_of(kana: &str, candidate: &str) -> bool {
    kana.is_kana() && !candidate.is_kana() && differs_only_where_one_writes_kanji(kana, candidate)
}

fn differs_only_where_one_writes_kanji(a: &str, b: &str) -> bool {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let prefix = a_chars.iter().zip(&b_chars).take_while(|(x, y)| x == y).count();
    let max_suffix = a_chars.len().min(b_chars.len()) - prefix;
    let suffix = a_chars
        .iter()
        .rev()
        .zip(b_chars.iter().rev())
        .take(max_suffix)
        .take_while(|(x, y)| x == y)
        .count();
    let mid_a: String = a_chars[prefix..a_chars.len() - suffix].iter().collect();
    let mid_b: String = b_chars[prefix..b_chars.len() - suffix].iter().collect();

    let kanji_vs_kana = |kanji: &str, kana: &str| {
        kanji.chars().any(is_kanji_char) && !kana.is_empty() && kana.is_kana()
    };
    kanji_vs_kana(&mid_a, &mid_b) || kanji_vs_kana(&mid_b, &mid_a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_answers_and_conflicts_do_not_merge() {
        assert_eq!(build_families(&["橋", "箸"], |_| None).len(), 2);
        assert_eq!(
            build_families(&["取付く", "取りつく", "取り憑く"], |s| Some(s.to_string())).len(),
            3
        );
        assert!(build_families(&[], |_| None).is_empty());
    }
    #[test]
    fn consistent_resolved_spellings_join() {
        assert_eq!(build_families(&["旨い", "上手い"], |_| Some("旨い".into())).len(), 1);
    }
    #[test]
    fn a_transitive_homophone_bridge_is_not_identity() {
        assert_eq!(variant_families(&["取り付く", "取りつく", "取り憑く"]).len(), 3);
    }
}

fn shared_tokenizer() -> Option<Arc<vibrato::Tokenizer>> {
    static TOKENIZER: OnceLock<Option<Arc<vibrato::Tokenizer>>> = OnceLock::new();
    TOKENIZER.get_or_init(|| init_vibrato(&DictType::Unidic, None).ok().map(Arc::new)).clone()
}

#[cfg(test)]
mod installed_evidence_tests {
    use super::*;
    use crate::dictionary::{
        frequency_dict::FrequencyDictionary,
        JsonFrequency,
        JsonFrequencyData,
        TermMetaBankV3,
    };
    #[test]
    fn disabled_marked_entries_and_cache_replacement() {
        let dictionary = FrequencyDictionary::new(
            "test".into(),
            "1".into(),
            vec![TermMetaBankV3 {
                term: "橋".into(),
                data_type: "freq".into(),
                data: Some(JsonFrequencyData::Nested {
                    reading: "ハシ".into(),
                    frequency: JsonFrequency::Complex {
                        value: 3,
                        display_value: Some("3㋕".into()),
                    },
                }),
            }],
        );
        let manager = FrequencyManager::from_dictionaries(vec![dictionary]);
        let initial = manager.lexical_families("はし");
        manager.set_dictionary_state("test", 0.0, false).unwrap();
        assert_eq!(manager.terms_with_reading_from_all_dictionaries("ハシ"), ["橋"]);
        assert!(Arc::ptr_eq(&initial, &manager.lexical_families("ハシ")));
        assert!(initial.family_for("橋").is_some());
        let replaced = FrequencyManager::from_dictionaries(vec![]);
        assert!(replaced.lexical_families("はし").families.is_empty());
    }
}
