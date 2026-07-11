//! Targeted N-best rescue: when the 1-best path yields a content word whose
//! (form, reading) no frequency dictionary corroborates, alternate
//! tokenizations of that span are tried and one whose words all validate is
//! spliced in. Worst case is always "1-best unchanged".

use vibrato::tokenizer::worker::Worker;
use wana_kana::{
    ConvertJapanese,
    IsJapaneseStr,
};

use super::{
    rule_matcher::parse_into_words,
    token_models::UnidicToken,
    unidic_tags::UnidicTag,
    word::{
        Word,
        POS,
    },
};
use crate::{
    core::utils::{
        is_kanji_char,
        pairwise_deinflection,
    },
    dictionary::frequency_manager::FrequencyManager,
};

const NBEST_PATHS: usize = 5;

pub fn rescue_words(
    worker: &mut Worker,
    sentence_text: &str,
    mut words: Vec<Word>,
    frequency_manager: &FrequencyManager,
) -> Vec<Word> {
    if frequency_manager.get_enabled_dictionaries().is_empty() {
        return words;
    }

    let flagged_spans: Vec<(usize, usize)> = (0..words.len())
        .filter(|&i| is_suspicious(&words, i, frequency_manager))
        .map(|i| word_span(&words[i]))
        .collect();
    if flagged_spans.is_empty() {
        return words;
    }

    worker.reset_sentence(sentence_text);
    worker.tokenize_nbest(NBEST_PATHS);
    let paths = collect_paths(worker, 0);

    for span in flagged_spans {
        // Splices shift indices, so re-locate the word by byte span each pass;
        // a span consumed by an earlier rescue no longer flags.
        let Some(idx) = (0..words.len())
            .find(|&i| word_span(&words[i]) == span && is_suspicious(&words, i, frequency_manager))
        else {
            continue;
        };
        let mut rescued = paths
            .iter()
            .any(|(_, tokens)| try_rescue_at(&mut words, idx, span, tokens, frequency_manager));
        // Whole-sentence paths may not vary at the flagged span; retry on the
        // span alone, then with one word of left context (つけた|けれ can only
        // re-fuse across the boundary).
        let mut fragments = vec![span];
        if idx > 0 {
            fragments.push((word_span(&words[idx - 1]).0, span.1));
        }
        for (start, end) in fragments {
            if rescued {
                break;
            }
            let Some(fragment) = sentence_text.get(start..end) else {
                continue;
            };
            worker.reset_sentence(fragment);
            worker.tokenize_nbest(NBEST_PATHS);
            for (_, tokens) in collect_paths(worker, start) {
                if try_rescue_at(&mut words, idx, span, &tokens, frequency_manager) {
                    rescued = true;
                    break;
                }
            }
        }
    }
    words
}

fn is_suspicious(words: &[Word], idx: usize, frequency_manager: &FrequencyManager) -> bool {
    needs_rescue(&words[idx], frequency_manager) || follows_ta_as_conditional(words, idx)
}

/// 助動詞タ directly followed by a bare verb in 仮定形 (つけた|けれ|ば) is
/// ungrammatical — a wrong boundary that validation alone can't flag.
fn follows_ta_as_conditional(words: &[Word], idx: usize) -> bool {
    idx > 0
        && words[idx - 1].tokens.last().is_some_and(|t| t.conjugation_type == UnidicTag::JodoushiTa)
        && words[idx].tokens.first().is_some_and(|t| {
            t.pos1 == UnidicTag::Doushi && t.conjugation_form == UnidicTag::Kateikei
        })
}

fn collect_paths(worker: &Worker, offset: usize) -> Vec<(i32, Vec<UnidicToken>)> {
    let mut paths: Vec<(i32, Vec<UnidicToken>)> = (0..worker.num_nbest_paths())
        .filter_map(|path_idx| {
            let cost = worker.path_cost(path_idx)?;
            let tokens = worker
                .nbest_token_iter(path_idx)?
                .map(|t| {
                    let range = t.range_byte();
                    UnidicToken::from_parts(
                        t.surface(),
                        t.feature(),
                        range.start + offset..range.end + offset,
                    )
                })
                .collect();
            Some((cost, tokens))
        })
        .collect();
    paths.sort_by_key(|(cost, _)| *cost);
    paths
}

fn needs_rescue(word: &Word, frequency_manager: &FrequencyManager) -> bool {
    rescue_eligible(word) && !validates(word, frequency_manager)
}

/// Content words a frequency dictionary is expected to back: kanji forms, or
/// kana forms of 3+ chars. Proper nouns and short kana words legitimately lack data.
fn rescue_eligible(word: &Word) -> bool {
    if !is_content(&word.part_of_speech)
        || word.tokens.iter().any(|t| t.pos2 == UnidicTag::Koyuumeishi)
    {
        return false;
    }
    let surface = effective_surface(word);
    contains_kanji(surface) || (surface.is_kana() && surface.chars().count() >= 3)
}

/// Attempt to replace the word at `idx` (flagged span `(s, e)`) with words
/// parsed from the alternate path's tokens. Returns true when spliced.
fn try_rescue_at(
    words: &mut Vec<Word>,
    idx: usize,
    (s, e): (usize, usize),
    path_tokens: &[UnidicToken],
    frequency_manager: &FrequencyManager,
) -> bool {
    let overlaps = |t: &UnidicToken| t.end_byte > s && t.start_byte < e;
    let Some(i0) = path_tokens.iter().position(overlaps) else {
        return false;
    };
    let mut j0 = i0;
    while j0 + 1 < path_tokens.len() && overlaps(&path_tokens[j0 + 1]) {
        j0 += 1;
    }
    let cover = (path_tokens[i0].start_byte, path_tokens[j0].end_byte);

    // The replaced range must line up with 1-best word boundaries, allowing at
    // most one extra word on each side of the flagged one.
    let (mut a, mut b) = (idx, idx);
    while cover.0 < word_span(&words[a]).0 {
        if a == 0 || idx - a >= 1 {
            return false;
        }
        a -= 1;
    }
    while cover.1 > word_span(&words[b]).1 {
        if b + 1 >= words.len() || b - idx >= 1 {
            return false;
        }
        b += 1;
    }
    let target = (word_span(&words[a]).0, word_span(&words[b]).1);

    // Widen the token range out to those word boundaries; if it still doesn't
    // tile exactly (a token crosses the boundary), reject this path.
    let mut i = i0;
    while i > 0 && path_tokens[i - 1].end_byte > target.0 {
        i -= 1;
    }
    let mut j = j0;
    while j + 1 < path_tokens.len() && path_tokens[j + 1].start_byte < target.1 {
        j += 1;
    }
    if (path_tokens[i].start_byte, path_tokens[j].end_byte) != target {
        return false;
    }

    let Ok(candidate) = parse_into_words(path_tokens[i..=j].to_vec()) else {
        return false;
    };
    if candidate.is_empty() {
        return false;
    }

    let all_content_words_validate =
        candidate.iter().all(|w| !needs_validation(w) || validates(w, frequency_manager));
    let flagged_span_now_validates = candidate.iter().any(|w| {
        let (ws, we) = word_span(w);
        ws < e && we > s && is_content(&w.part_of_speech) && validates(w, frequency_manager)
    });
    if !all_content_words_validate
        || !flagged_span_now_validates
        || words_equal(&words[a..=b], &candidate)
    {
        return false;
    }

    words.splice(a..=b, candidate);
    true
}

fn needs_validation(word: &Word) -> bool {
    rescue_eligible(word)
}

fn is_content(pos: &POS) -> bool {
    matches!(
        pos,
        POS::Noun
            | POS::Verb
            | POS::SuruVerb
            | POS::Adjective
            | POS::AdjectivalNoun
            | POS::Suffix
            | POS::Prefix
    )
}

/// The surface the pipeline will mine from this word (mirrors `Term::from`).
fn effective_surface(word: &Word) -> &str {
    match &word.main_word {
        Some(main) => &main.surface,
        None => &word.surface_form,
    }
}

/// True when some frequency dictionary corroborates the word's (form, reading)
/// — via any deinflection candidate or the UniDic lemma pair.
fn validates(word: &Word, frequency_manager: &FrequencyManager) -> bool {
    let (surface, reading, lemma, lemma_reading) = match &word.main_word {
        Some(m) => (&m.surface, &m.surface_hatsuon, &m.lemma_form, &m.lemma_hatsuon),
        None => (&word.surface_form, &word.surface_hatsuon, &word.lemma_form, &word.lemma_hatsuon),
    };
    pairwise_deinflection(surface, reading)
        .iter()
        .any(|(w, r)| frequency_manager.get_harmonic_frequency_for_pair(w, r).is_some())
        || frequency_manager
            .get_harmonic_frequency_for_pair(lemma, &lemma_reading.to_hiragana())
            .is_some()
}

fn word_span(word: &Word) -> (usize, usize) {
    word.byte_span()
}

fn contains_kanji(s: &str) -> bool {
    s.chars().any(is_kanji_char)
}

fn words_equal(a: &[Word], b: &[Word]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.surface_form == y.surface_form
                && x.surface_hatsuon == y.surface_hatsuon
                && x.lemma_form == y.lemma_form
                && x.lemma_hatsuon == y.lemma_hatsuon
                && x.part_of_speech == y.part_of_speech
        })
}
