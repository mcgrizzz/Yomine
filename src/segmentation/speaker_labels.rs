//! Subtitle speaker labels: `（伊黒）` before a line names who speaks; `（足音）` describes a sound.
use std::collections::HashSet;

use crate::{
    core::{
        models::{
            Sentence,
            Term,
        },
        utils::is_kanji_char,
    },
    dictionary::frequency_manager::FrequencyManager,
    segmentation::word::POS,
};

/// Subtitle labels: a bracketed group opening the text or following whitespace, with any
/// dialogue dash before it (`-（伊黒）`, `（足音）`). Each is its whole byte range and the
/// byte range inside the brackets.
pub(super) fn bracket_labels(text: &str) -> Vec<((usize, usize), (usize, usize))> {
    let mut labels = Vec::new();
    let mut line_start = true;
    let mut dash_start = None;
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            _ if c.is_whitespace() => {
                line_start = true;
                dash_start = None;
            }
            '-' | '－' | '‐' | '―' if line_start => {
                dash_start.get_or_insert(i);
            }
            '（' | '(' if line_start => {
                let close = if c == '（' { '）' } else { ')' };
                match chars.by_ref().find(|&(_, c)| c == close) {
                    Some((j, _)) => labels.push((
                        (dash_start.unwrap_or(i), j + close.len_utf8()),
                        (i + c.len_utf8(), j),
                    )),
                    None => break,
                }
                line_start = false;
                dash_start = None;
            }
            _ => {
                line_start = false;
                dash_start = None;
            }
        }
    }
    labels
}

/// A label naming the speaker rather than describing a sound (（足音）, （炭治郎の声）):
/// katakana UniDic doesn't know (anime frequency lists rank names like フリーレン), proper
/// nouns only, or kanji no frequency list knows (UniDic splits 伊黒 into 伊 + 黒).
pub(super) fn names_speaker(content: &str, words: &[&Term], manager: &FrequencyManager) -> bool {
    let katakana = |c: char| matches!(c, 'ァ'..='ヺ' | 'ー' | '・' | '･' | 'ｦ'..='ﾟ');
    !content.is_empty()
        && ((content.chars().all(katakana) && words.iter().all(|w| w.lexeme.is_none()))
            || (!words.is_empty() && words.iter().all(|w| w.part_of_speech == POS::ProperNoun))
            || (content.chars().all(is_kanji_char)
                && manager.get_frequency_data_by_term(content).is_empty()))
}

/// Dialogue names its speakers too (善逸！), where UniDic splits the name into words; a term
/// that only ever starts inside such a name is skipped by auto mode, suffix and all (黒さん
/// in 伊黒さん).
pub(super) fn mark_speaker_names(
    terms: &mut [Term],
    sentences: &[Sentence],
    names: &HashSet<String>,
) {
    for term in terms {
        term.auto_skip.speaker_name = !term.sentence_references.is_empty()
            && term.sentence_references.iter().all(|&(ord, start)| {
                let text = &sentences[ord].text;
                names.iter().any(|name| {
                    text.match_indices(name.as_str())
                        .any(|(at, _)| (at..at + name.len()).contains(&start))
                })
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segmentation::tokenizer::extract_words;

    #[test]
    fn a_name_from_a_speaker_label_is_flagged_in_dialogue() {
        let Some(tokenizer) = crate::segmentation::lexeme_resolver::test_tokenizer() else {
            return;
        };
        let manager = FrequencyManager::from_dictionaries(vec![]);
        let mut sentences: Vec<Sentence> =
            ["（善逸）うるさいな", "（伊黒）のろい", "善逸！ 伊黒さん 待って"]
                .iter()
                .enumerate()
                .map(|(id, text)| Sentence {
                    id,
                    source_id: 0,
                    text: text.to_string(),
                    segments: vec![],
                    timestamp: None,
                    comprehension: 0.0,
                })
                .collect();
        let terms = extract_words(tokenizer.new_worker(), &mut sentences, &manager);
        let flagged = |form: &str| {
            terms.iter().find(|t| t.surface_form == form).map(|t| t.auto_skip.speaker_name)
        };
        assert_eq!(flagged("善"), Some(true));
        assert_eq!(flagged("黒さん"), Some(true));
        assert_eq!(flagged("待っ").or(flagged("待って")), Some(false));
    }
}
