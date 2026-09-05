//! Explicit Anki match outcomes; comprehension is computed only for Known.
use super::types::Vocab;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchEvidence {
    ExactSurface,
    Citation,
    LexicalFamily,
}
pub enum MatchResult<'a> {
    Known { card: &'a Vocab, evidence: MatchEvidence },
    Possible { card: &'a Vocab },
    Unmatched,
}
