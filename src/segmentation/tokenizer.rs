use std::collections::HashMap;

use vibrato::{
    tokenizer::worker::Worker,
    Tokenizer,
};
use wana_kana::IsJapaneseStr;

use super::{
    nbest::rescue_words,
    rule_matcher::parse_into_words,
    token_models::UnidicToken,
    unidic_tags::UnidicTag,
    word::{
        get_default_pos,
        Citation,
        CitationProvenance,
        Word,
        POS,
    },
};
use crate::{
    core::{
        utils::{
            is_kanji_char,
            normalize_reading,
            pairwise_deinflection,
            NormalizeLongVowel,
        },
        Sentence,
        Term,
        YomineError,
    },
    dictionary::{
        frequency_manager::{
            fold_katakana,
            FrequencyManager,
        },
        jmdict_lexicon,
        token_dictionary::{
            load_dictionary,
            DictType,
        },
    },
};

// Lexical exceptions are intentionally reading- and spelling-constrained.
const LEXICAL_CITATIONS: &[(&str, &str)] = &[("つまらない", "つまらない")];

pub(super) fn resolve_citation(word: &mut Word, manager: &FrequencyManager) {
    if word.citation.is_some()
        || word.main_word.is_some()
        || !word.surface_form.as_str().is_japanese()
    {
        return;
    }
    if !matches!(
        word.part_of_speech,
        POS::Verb | POS::SuruVerb | POS::Adjective | POS::AdjectivalNoun | POS::Noun
    ) {
        return;
    }
    let deinflections = pairwise_deinflection(&word.surface_form, &word.surface_hatsuon);
    let exception = deinflections
        .iter()
        .find(|(form, reading)| {
            word.surface_form.as_str().is_kana()
                && LEXICAL_CITATIONS.contains(&(form.as_str(), reading.as_str()))
        })
        .cloned();
    let mut candidates: Vec<_> = deinflections
        .into_iter()
        .filter(|(form, reading)| manager.get_harmonic_frequency_for_pair(form, reading).is_some())
        .collect();
    candidates
        .sort_by_key(|(form, reading)| manager.get_harmonic_frequency_for_pair(form, reading));
    let lemma = (word.lemma_form.clone(), word.lemma_hatsuon.clone());
    match word.part_of_speech {
        POS::Verb => retain_verb_final_candidates(&mut candidates, &word.surface_form),
        // An adjective stem can deinflect as a verb (のろ as 乗る's imperative).
        POS::Adjective => candidates.retain(|pair| pair.0.ends_with('い') || *pair == lemma),
        _ => {}
    }
    let selected = exception
        .map(|pair| (pair, CitationProvenance::LexicalException))
        .or_else(|| {
            candidates
                .iter()
                .find(|pair| **pair == lemma)
                .or(candidates.first())
                .cloned()
                .map(|pair| (pair, CitationProvenance::ValidatedDeinflection))
        })
        .or_else(|| {
            let [head, tail @ ..] = word.tokens.as_slice() else {
                return None;
            };
            (!tail.is_empty()
                && tail.iter().all(|t| t.pos1 == super::unidic_tags::UnidicTag::Jodoushi))
            .then(|| {
                (
                    (
                        head.lemma_form.clone(),
                        normalize_reading(&head.surface, &head.lemma_hatsuon),
                    ),
                    CitationProvenance::AuxiliaryHead,
                )
            })
        });
    if let Some(((form, reading), provenance)) = selected {
        word.lemma_form = form.clone();
        word.lemma_hatsuon = reading.clone();
        word.citation = Some(Citation { form, reading, provenance });
    }
}

fn analyze_sentence(worker: &mut Worker, text: &str, manager: &FrequencyManager) -> Vec<Word> {
    worker.reset_sentence(text);
    worker.tokenize();
    let tokens = worker
        .token_iter()
        .map(|token| UnidicToken::from_parts(token.surface(), token.feature(), token.range_byte()))
        .collect();
    let words = parse_into_words(tokens).unwrap_or_default();
    let words = reparse_katakana_speech(worker, words, manager);
    let mut words =
        split_unvalidated_compounds(rescue_words(worker, text, words, manager), manager);
    keep_lexicalized_honorifics(&mut words, manager);
    tag_katakana_interjections(worker, &mut words);
    words
}

/// Some characters speak in katakana (ウルサクナイモンネ), which UniDic can't read and
/// leaves as unknown tokens. Read as hiragana, speech parses into dictionary words ending
/// in a sentence-final particle; names and loanwords (シルフィエット, アウトドアカレー)
/// are known tokens or don't end that way, so they are never reread.
fn reparse_katakana_speech(
    worker: &mut Worker,
    words: Vec<Word>,
    manager: &FrequencyManager,
) -> Vec<Word> {
    let mut out = Vec::with_capacity(words.len());
    for word in words {
        match katakana_as_hiragana(worker, &word, manager) {
            Some(reread) => out.extend(reread),
            None => out.push(word),
        }
    }
    out
}

fn katakana_as_hiragana(
    worker: &mut Worker,
    word: &Word,
    manager: &FrequencyManager,
) -> Option<Vec<Word>> {
    let surface = word.surface_form.as_str();
    // An unknown token's lexeme falls back to its own surface.
    let all_unknown = word.tokens.iter().all(|t| t.lexeme == t.surface);
    if !surface.is_katakana()
        || surface.chars().count() < 3
        || !all_unknown
        || !manager.get_frequency_data_by_term(surface).is_empty()
    {
        return None;
    }
    // Folding keeps every character three bytes wide, so offsets carry over.
    worker.reset_sentence(&fold_katakana(surface));
    worker.tokenize();
    let start = word.byte_span().0;
    let mut tokens = Vec::new();
    for token in worker.token_iter() {
        let fields: Vec<&str> = token.feature().split(',').collect();
        let listed = fields.get(7).is_some_and(|lemma| *lemma != "*");
        if !listed || fields[0] == "補助記号" || fields.get(1) == Some(&"固有名詞") {
            return None;
        }
        let range = token.range_byte();
        tokens.push(UnidicToken::from_parts(
            token.surface(),
            token.feature(),
            start + range.start..start + range.end,
        ));
    }
    // Speech has a predicate and ends like speech (ナイモンネ, クルワヨ); a name that
    // happens to end in な or ね (ハンナ) has no predicate.
    let ends_as_speech = tokens.last().is_some_and(|t| t.pos2 == UnidicTag::Shuujoshi);
    let has_predicate = tokens
        .iter()
        .any(|t| matches!(t.pos1, UnidicTag::Doushi | UnidicTag::Keiyoushi | UnidicTag::Jodoushi));
    if tokens.len() < 2 || !ends_as_speech || !has_predicate {
        return None;
    }
    // Parsed with hiragana surfaces so the katakana binding rule can't re-merge them.
    let mut reread = parse_into_words(tokens).ok()?;
    let original = |t: &UnidicToken| surface[t.start_byte - start..t.end_byte - start].to_string();
    for w in &mut reread {
        for t in &mut w.tokens {
            t.surface = original(t);
        }
        if let Some(main) = &mut w.main_word {
            main.surface = original(main);
        }
        w.surface_form = w.tokens.iter().map(|t| t.surface.as_str()).collect();
    }
    Some(reread)
}

/// UniDic's first reading of a lone katakana exclamation can be a noun (フン as the
/// Huns or as 糞), and an unknown one defaults to a noun (ンン). No word begins with
/// ン or ッ.
fn tag_katakana_interjections(worker: &mut Worker, words: &mut [Word]) {
    let bounded = |i: usize| words.get(i).is_none_or(|w| w.part_of_speech == POS::Symbol);
    let standalone: Vec<bool> =
        (0..words.len()).map(|i| (i == 0 || bounded(i - 1)) && bounded(i + 1)).collect();
    for (word, standalone) in words.iter_mut().zip(standalone) {
        let [token] = word.tokens.as_slice() else {
            continue;
        };
        if !token.surface.as_str().is_katakana() {
            continue;
        }
        let unknown_exclamation =
            token.lexeme == token.surface && token.surface.starts_with(['ン', 'ッ']);
        let exclaimed_noun = standalone
            && token.pos1 == super::unidic_tags::UnidicTag::Meishi
            && has_interjection_reading(worker, &token.surface);
        if unknown_exclamation || exclaimed_noun {
            word.part_of_speech = POS::Interjection;
        }
    }
}

/// A dictionary interjection entry for this spelling. Unknown-word guesses don't count:
/// UniDic offers 感動詞 for any unknown katakana (エリス), with no lemma.
fn has_interjection_reading(worker: &mut Worker, surface: &str) -> bool {
    worker.reset_sentence(surface);
    worker.tokenize_nbest(5);
    (0..worker.num_nbest_paths()).any(|path| {
        worker.nbest_token_iter(path).is_some_and(|mut tokens| {
            let (Some(token), None) = (tokens.next(), tokens.next()) else {
                return false;
            };
            let fields: Vec<&str> = token.feature().split(',').collect();
            fields.first() == Some(&"感動詞") && fields.get(7).is_some_and(|lemma| *lemma != "*")
        })
    })
}

/// The honorific rule mines the noun after お/ご (嬢 in お嬢様). A prefixed word the
/// dictionaries list is a word of its own, so it stays whole, read the way they
/// mostly read it: UniDic reads 兄 in お兄さん as あに, and BCCWJ inherits that.
fn keep_lexicalized_honorifics(words: &mut [Word], manager: &FrequencyManager) {
    for word in words {
        let (Some(prefix), Some(_)) = (word.tokens.first(), word.main_word.as_ref()) else {
            continue;
        };
        if prefix.pos1 != super::unidic_tags::UnidicTag::Settouji
            || !matches!(prefix.surface.as_str(), "お" | "ご" | "御")
        {
            continue;
        }
        let reading = manager
            .majority_reading(&word.surface_form)
            .map(|reading| as_written(&word.surface_hatsuon, reading))
            .or_else(|| {
                phrase_frequency(manager, &word.surface_form, &word.surface_hatsuon)
                    .map(|_| word.surface_hatsuon.clone())
            });
        if let Some(reading) = reading {
            word.main_word = None;
            word.lemma_form = word.surface_form.clone();
            word.surface_hatsuon = reading.clone();
            word.lemma_hatsuon = reading;
        }
    }
}

#[derive(Clone, Copy)]
enum PhraseMode {
    Production,
    Corpus,
}

impl PhraseMode {
    fn readings(self, terms: &[Term]) -> Vec<String> {
        match self {
            Self::Production => phrase_reading_candidates(terms),
            // Corpus analysis keeps surface-first candidates and deinflects later.
            Self::Corpus => vec![terms.iter().map(|t| t.full_segment_reading.as_str()).collect()],
        }
    }
}

/// Subtitle labels: a bracketed group opening the text or following whitespace, with any
/// dialogue dash before it (`-（伊黒）`, `（足音）`). Each is its whole byte range and the
/// byte range inside the brackets.
fn bracket_labels(text: &str) -> Vec<((usize, usize), (usize, usize))> {
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
fn names_speaker(content: &str, words: &[&Term], manager: &FrequencyManager) -> bool {
    let katakana = |c: char| matches!(c, 'ァ'..='ヺ' | 'ー' | '・' | '･' | 'ｦ'..='ﾟ');
    !content.is_empty()
        && ((content.chars().all(katakana) && words.iter().all(|w| w.lexeme.is_none()))
            || (!words.is_empty() && words.iter().all(|w| w.part_of_speech == POS::ProperNoun))
            || (content.chars().all(is_kanji_char)
                && manager.get_frequency_data_by_term(content).is_empty()))
}

fn phrase_frequency(manager: &FrequencyManager, form: &str, reading: &str) -> Option<u32> {
    manager.get_harmonic_frequency_for_pair(
        &form.normalize_long_vowel(),
        &reading.normalize_long_vowel(),
    )
}

/// Dictionaries store long-vowel-normalized readings (おねいさん); keep the text's own
/// spelling when it is the same reading.
fn as_written(ours: &str, dictionary: String) -> String {
    if ours.normalize_long_vowel() == dictionary.normalize_long_vowel() {
        ours.to_string()
    } else {
        dictionary
    }
}

/// Construct candidates independently of the caller's acceptance and suppression policy.
fn phrase_candidate(
    subrange: &[Term],
    manager: &FrequencyManager,
    mode: PhraseMode,
) -> (Term, Option<u32>) {
    let mut phrase = Term::from_slice(subrange);
    let readings = mode.readings(subrange);
    if matches!(mode, PhraseMode::Production) {
        for reading in &readings {
            let evidence = manager.lexical_families(reading);
            if let Some(family) = evidence.phrase_family(&phrase.surface_form) {
                phrase.lexical_family = Some(family);
                break;
            }
        }
    }
    let mut frequency = None;
    for reading in readings {
        if let Some(rank) = phrase_frequency(manager, &phrase.surface_form, &reading) {
            phrase.surface_reading = reading.clone();
            phrase.lemma_reading = reading.clone();
            phrase.full_segment_reading = reading;
            frequency = Some(rank);
            break;
        }
    }
    // UniDic can read a component differently inside a compound (表 as ひょう, but
    // 表沙汰 is おもてざた), so a spelling every dictionary reads one way takes that reading.
    let kanji_nouns = subrange
        .iter()
        .all(|t| matches!(t.part_of_speech, POS::Noun | POS::CompoundNoun | POS::ProperNoun))
        && phrase.surface_form.chars().all(is_kanji_char);
    if frequency.is_none() && matches!(mode, PhraseMode::Production) && kanji_nouns {
        let sole = manager.sole_reading(&phrase.surface_form).and_then(|reading| {
            let reading = as_written(&phrase.full_segment_reading, reading);
            phrase_frequency(manager, &phrase.surface_form, &reading).map(|rank| (reading, rank))
        });
        if let Some((reading, rank)) = sole {
            phrase.surface_reading = reading.clone();
            phrase.lemma_reading = reading.clone();
            phrase.full_segment_reading = reading;
            frequency = Some(rank);
        }
    }
    if frequency.is_none() {
        if let Some(family) = &phrase.lexical_family {
            for spelling in &family.spellings {
                if let Some(rank) =
                    manager.get_harmonic_frequency_for_pair(spelling, &phrase.lemma_reading)
                {
                    phrase.lemma_form = spelling.clone();
                    frequency = Some(rank);
                    break;
                }
            }
        }
    }
    // The full surface remains the highlight; only the citation takes the lemma.
    if frequency.is_none() {
        if let Some(citation) = citation_form_subrange(subrange) {
            let form: String = citation.iter().map(|t| t.full_segment.as_str()).collect();
            for reading in mode.readings(&citation) {
                if let Some(rank) = phrase_frequency(manager, &form, &reading) {
                    phrase.lemma_form = form.clone();
                    phrase.lemma_reading = reading;
                    frequency = Some(rank);
                    break;
                }
            }
        }
    }
    (phrase, frequency)
}

pub fn extract_words(
    mut worker: Worker,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
) -> Vec<Term> {
    let mut terms = Vec::<Term>::new();

    for (ord, sentence) in sentences.iter_mut().enumerate() {
        let words = analyze_sentence(&mut worker, &sentence.text, frequency_manager);

        let mut term_spans: Vec<(usize, usize)> = Vec::with_capacity(words.len());
        let mut sentence_terms: Vec<Term> = Vec::with_capacity(words.len());
        let mut rule_citations = Vec::with_capacity(words.len());
        for word in words {
            let span = word.byte_span();
            rule_citations.push(word.has_rule_citation());
            // The highlight span ends at start + surface_form.len(), so the
            // reference must point at the main word, not the segment.
            let ref_start = word.mining_span().0;
            let mut term: Term = word.into();
            let freq_map: HashMap<String, u32> = frequency_manager.build_freq_map(
                &term.lemma_form,
                &term.lemma_reading,
                term.is_kana,
            );
            term.frequencies = freq_map;

            term.sentence_references.push((ord, ref_start));
            term_spans.push(span);
            sentence_terms.push(term);
        }

        sentence.segments.extend(sentence_terms.iter().zip(&term_spans).map(
            |(term, &(start_index, end_index))| {
                // Furigana needs the full segment reading, including attached auxiliaries.
                (
                    term.full_segment_reading.clone(),
                    term.part_of_speech.clone(),
                    start_index,
                    end_index,
                )
            },
        ));

        let base_len = sentence_terms.len();
        let labels: Vec<(usize, usize)> = bracket_labels(&sentence.text)
            .into_iter()
            .filter(|&(_, (start, end))| {
                let words: Vec<&Term> = sentence_terms
                    .iter()
                    .zip(&term_spans)
                    .filter(|(t, &(s, e))| {
                        s >= start && e <= end && !t.surface_form.trim().is_empty()
                    })
                    .map(|(t, _)| t)
                    .collect();
                names_speaker(&sentence.text[start..end], &words, frequency_manager)
            })
            .map(|(label, _)| label)
            .collect();
        let labelled: Vec<bool> = term_spans
            .iter()
            .map(|&(s, e)| labels.iter().any(|&(ls, le)| s >= ls && e <= le))
            .collect();
        // A kana noun right before an ellipsis is usually a word cut off mid-way
        // (お願いしま… as 縞).
        let cut_off = |i: usize| {
            sentence_terms[i].is_kana
                && sentence_terms[i].part_of_speech == POS::Noun
                && sentence_terms
                    .get(i + 1)
                    .is_some_and(|n| n.surface_form.starts_with(['…', '‥', '.', '．']))
        };
        // Whitespace, speaker labels and cut-off words keep their display segments but are
        // never terms.
        let mut suppressed: Vec<bool> = (0..base_len)
            .map(|i| labelled[i] || cut_off(i) || sentence_terms[i].surface_form.trim().is_empty())
            .collect();
        // A phrase that hides its words also hides phrases inside it (にかけて in 気にかける).
        let mut hidden_through = None;
        for start in 0..base_len {
            if hidden_through.is_some_and(|last| start <= last) {
                continue;
            }
            for end in (start + 1..base_len).rev() {
                if labelled[start..=end].iter().any(|&l| l) {
                    continue;
                }
                let subrange = &sentence_terms[start..=end];
                // Frequency lists carry particle n-grams (あなたに); JMdict lists only real
                // phrases that begin or end in one (ついでに, にとって).
                let listed_particle_phrase = || {
                    subrange.iter().any(phrase_endpoint_ok)
                        && jmdict_lexicon::is_phrase(
                            &subrange.iter().map(|t| t.full_segment.as_str()).collect::<String>(),
                        )
                };
                if (!phrase_endpoint_ok(&subrange[0])
                    || !phrase_endpoint_ok(&subrange[end - start]))
                    && !listed_particle_phrase()
                {
                    continue;
                }
                let (mut phrase, freq) =
                    phrase_candidate(subrange, frequency_manager, PhraseMode::Production);

                if let Some(frequency) = freq {
                    let word_frequencies: Vec<(String, f32)> = subrange
                        .iter()
                        .map(|term| {
                            let freq = term
                                .frequencies
                                .get("HARMONIC")
                                .cloned()
                                .unwrap_or(u32::max_value());
                            (term.lemma_form.to_string(), freq as f32)
                        })
                        .collect();

                    let mult_frequencies: f32 =
                        word_frequencies.iter().map(|(_, freq)| *freq as f32).product();

                    let k = subrange.len() as u32;
                    let score: f32 = (frequency as f32).powf(k as f32) / mult_frequencies;

                    let ratios: Vec<f32> = word_frequencies
                        .iter()
                        .map(|(_, freq)| (frequency as f32) / *freq)
                        .collect();

                    let max_ratio: f32 = ratios.iter().fold(0.0, |acc, &x| acc.max(x));
                    let char_count = phrase.lemma_form.chars().count();

                    let all_nouns = subrange.iter().all(|term| {
                        matches!(
                            term.part_of_speech,
                            POS::Noun | POS::CompoundNoun | POS::ProperNoun
                        )
                    });
                    let all_content_words = subrange.iter().all(phrase_endpoint_ok);

                    let score_threshold = 10.0;
                    let ratio_threshold = 120.0;
                    // The score/ratio gates guard against junk n-gram entries
                    // some dictionaries carry; all-kanji noun compounds with
                    // an exact reading match (複合体) don't need them.
                    let kanji_noun_compound =
                        all_nouns && phrase.lemma_form.chars().all(is_kanji_char);
                    let min_len = if kanji_noun_compound { 3 } else { 4 };

                    let override_ratio_threshold = 40.0;
                    let phrase_freq_threshold = 10000;

                    if char_count < min_len {
                        continue;
                    }

                    // A component no dictionary knows (負け+じと's じと) means the
                    // 1-best parse is itself suspect — the corroborated phrase wins.
                    let has_unvalidated_component =
                        word_frequencies.iter().any(|(_, freq)| *freq == u32::MAX as f32);
                    // A JMdict phrase is no junk n-gram, and it reads as one unit even
                    // around a particle (しょうがない).
                    let listed = jmdict_lexicon::is_phrase(&phrase.surface_form)
                        || jmdict_lexicon::is_phrase(&phrase.lemma_form);

                    if !kanji_noun_compound
                        && !has_unvalidated_component
                        && !listed
                        && frequency > phrase_freq_threshold
                        && max_ratio < override_ratio_threshold
                    {
                        continue;
                    }

                    phrase.part_of_speech =
                        if all_nouns { POS::NounExpression } else { POS::Expression };

                    if kanji_noun_compound
                        || listed
                        || score <= score_threshold
                        || max_ratio >= ratio_threshold
                    {
                        phrase.sentence_references.push((ord, term_spans[start].0));
                        let freq_map = frequency_manager.build_freq_map(
                            &phrase.lemma_form,
                            &phrase.lemma_reading,
                            phrase.is_kana,
                        );
                        phrase.frequencies = freq_map;
                        sentence_terms.push(phrase);

                        if (all_content_words || listed)
                            && !rule_citations[start..=end].iter().any(|v| *v)
                        {
                            for flag in suppressed[start..=end].iter_mut() {
                                *flag = true;
                            }
                            hidden_through = Some(end);
                        }

                        // Largest phrase at this start position is accepted; move to next start.
                        break;
                    }
                }
            }
        }

        let mut idx = 0;
        sentence_terms.retain(|_| {
            let keep = idx >= base_len || !suppressed[idx];
            idx += 1;
            keep
        });

        for term in terms.iter_mut() {
            for sentence_term in sentence_terms.iter().filter(|st| {
                st.lemma_form == term.lemma_form && st.lemma_reading == term.lemma_reading
            }) {
                for sentence_ref in &sentence_term.sentence_references {
                    if !term.sentence_references.contains(sentence_ref) {
                        term.sentence_references.push(*sentence_ref);
                    }
                }
            }
        }

        sentence_terms.retain(|sentence_term| {
            !terms.iter().any(|existing_term| {
                existing_term.lemma_form == sentence_term.lemma_form
                    && existing_term.lemma_reading == sentence_term.lemma_reading
            })
        });

        terms.append(&mut sentence_terms);
    }

    terms
}

pub fn extract_words_for_frequency(
    tokenizer: &Tokenizer,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
    progress_callback: Option<&(dyn Fn(bool, usize, usize) + Sync)>,
) -> Vec<Term> {
    use std::sync::atomic::{
        AtomicUsize,
        Ordering,
    };

    use rayon::prelude::*;

    let total_sentences = sentences.len();
    let report_interval = std::cmp::min(1000, std::cmp::max(1, total_sentences / 20));
    let processed_count = AtomicUsize::new(0);

    let all_terms: Vec<Vec<Term>> = sentences
        .par_iter()
        .map_init(
            || tokenizer.new_worker(),
            |worker, sentence| {
                let words = analyze_sentence(worker, &sentence.text, frequency_manager);

                let mut sentence_terms: Vec<Term> = words
                    .into_iter()
                    .filter_map(|word| {
                        let mut term: Term = word.into();

                        // Filter out blank/whitespace tokens
                        if term.surface_form.trim().is_empty() {
                            return None;
                        }

                        // For frequency analysis, use surface forms as lemma forms initially
                        // Deinflection will be applied in batch later
                        term.lemma_form = term.surface_form.clone();
                        term.lemma_reading = term.surface_reading.clone();

                        Some(term)
                    })
                    .collect();

                // Phrase detection - check if phrase exists in loaded dictionaries
                // Adds the largest matching phrase at each starting position.
                let base_len = sentence_terms.len();
                for start in 0..base_len {
                    for end in (start + 1..base_len).rev() {
                        let subrange = &sentence_terms[start..=end];
                        if !phrase_endpoint_ok(&subrange[0])
                            || !phrase_endpoint_ok(&subrange[end - start])
                        {
                            continue;
                        }
                        let (mut phrase, freq) =
                            phrase_candidate(subrange, frequency_manager, PhraseMode::Corpus);

                        if let Some(_frequency) = freq {
                            let char_count = phrase.lemma_form.chars().count();
                            let all_nouns = subrange.iter().all(|term| {
                                matches!(
                                    term.part_of_speech,
                                    POS::Noun | POS::CompoundNoun | POS::ProperNoun
                                )
                            });
                            let min_len =
                                if all_nouns && phrase.lemma_form.chars().all(is_kanji_char) {
                                    3
                                } else {
                                    4
                                };

                            if char_count < min_len {
                                continue;
                            }

                            phrase.part_of_speech =
                                if all_nouns { POS::NounExpression } else { POS::Expression };

                            sentence_terms.push(phrase);
                            // Move to the next start position.
                            break;
                        }
                    }
                }

                // Update progress counter (phase = true)
                let current = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(ref callback) = progress_callback {
                    if current % report_interval == 0 || current == total_sentences {
                        callback(true, current, total_sentences);
                    }
                }

                sentence_terms
            },
        )
        .collect();

    let terms: Vec<Term> = all_terms.into_iter().flatten().collect();

    terms
}

pub fn batch_deinflect_terms(
    terms: &[Term],
    frequency_manager: &FrequencyManager,
) -> HashMap<(String, String), (String, String)> {
    use std::collections::HashSet;

    use rayon::prelude::*;

    // Collect unique (surface_form, surface_reading, POS) tuples
    let mut unique_surfaces: HashSet<(String, String, POS)> = HashSet::new();
    for term in terms {
        if term.surface_form.as_str().is_japanese() {
            unique_surfaces.insert((
                term.surface_form.clone(),
                term.surface_reading.clone(),
                term.part_of_speech.clone(),
            ));
        }
    }

    // Deinflect unique surfaces
    let deinflection_map: HashMap<(String, String), (String, String)> = unique_surfaces
        .par_iter()
        .map(|(surface_form, surface_reading, pos)| {
            let (lemma_form, lemma_reading) = match pos {
                POS::Verb
                | POS::SuruVerb
                | POS::AdjectivalNoun
                | POS::Adjective
                | POS::NounExpression
                | POS::Expression
                | POS::Noun => {
                    let deinflections: Vec<(String, String)> =
                        pairwise_deinflection(surface_form, surface_reading);

                    let mut sorted_deinflections: Vec<(String, String)> = deinflections
                        .into_iter()
                        .filter(|(word, reading)| {
                            frequency_manager
                                .get_harmonic_frequency_for_pair(word, reading)
                                .is_some()
                        })
                        .collect();

                    sorted_deinflections.sort_by_key(|(word, reading)| {
                        frequency_manager.get_harmonic_frequency_for_pair(word, reading)
                    });

                    if *pos == POS::Verb {
                        retain_verb_final_candidates(&mut sorted_deinflections, surface_form);
                    }

                    if !sorted_deinflections.is_empty() {
                        sorted_deinflections[0].clone()
                    } else {
                        // No valid deinflection found, use surface form
                        (surface_form.clone(), surface_reading.clone())
                    }
                }
                _ => {
                    // Other POS types don't need deinflection
                    (surface_form.clone(), surface_reading.clone())
                }
            };

            ((surface_form.clone(), surface_reading.clone()), (lemma_form, lemma_reading))
        })
        .collect();

    deinflection_map
}

// Verb dictionary forms always end in u-row kana; the ない/surface arms keep lexicalized words (つまらない).
fn retain_verb_final_candidates(candidates: &mut Vec<(String, String)>, surface: &str) {
    let verbish: Vec<(String, String)> = candidates
        .iter()
        .filter(|(word, _)| {
            word.chars().last().is_some_and(|c| "うくぐすつぬぶむる".contains(c))
                || word.ends_with("ない")
                || word == surface
        })
        .cloned()
        .collect();
    if !verbish.is_empty() {
        *candidates = verbish;
    }
}

fn phrase_endpoint_ok(term: &Term) -> bool {
    !matches!(term.part_of_speech, POS::Postposition | POS::Copula | POS::Symbol)
}

/// Split rule-merged proper+common compounds back apart (夏子おばさん →
/// 夏子 + おばさん) unless a frequency dictionary corroborates the merged pair.
fn split_unvalidated_compounds(
    words: Vec<Word>,
    frequency_manager: &FrequencyManager,
) -> Vec<Word> {
    if frequency_manager.get_enabled_dictionaries().is_empty() {
        return words;
    }
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for word in words {
        if word.part_of_speech != POS::CompoundNoun
            || word.tokens.len() < 2
            || frequency_manager
                .get_harmonic_frequency_for_pair(&word.surface_form, &word.surface_hatsuon)
                .is_some()
        {
            out.push(word);
            continue;
        }
        let Ok(rest_words) = parse_into_words(word.tokens[1..].to_vec()) else {
            out.push(word);
            continue;
        };
        let first = &word.tokens[0];
        out.push(Word {
            citation: None,
            surface_form: first.surface.clone(),
            surface_hatsuon: normalize_reading(&first.surface, &first.surface_hatsuon),
            lemma_form: first.lemma_form.clone(),
            lemma_hatsuon: normalize_reading(&first.surface, &first.lemma_hatsuon),
            part_of_speech: get_default_pos(first),
            tokens: vec![first.clone()],
            main_word: None,
        });
        out.extend(rest_words);
    }
    out
}

/// Dakuten/handakuten variants of a leading kana — the sequential-voicing
/// (rendaku) alternations a non-initial compound component can take.
fn rendaku_variants(c: char) -> &'static [char] {
    match c {
        'か' => &['が'],
        'き' => &['ぎ'],
        'く' => &['ぐ'],
        'け' => &['げ'],
        'こ' => &['ご'],
        'さ' => &['ざ'],
        'し' => &['じ'],
        'す' => &['ず'],
        'せ' => &['ぜ'],
        'そ' => &['ぞ'],
        'た' => &['だ'],
        'ち' => &['ぢ'],
        'つ' => &['づ'],
        'て' => &['で'],
        'と' => &['ど'],
        'は' => &['ば', 'ぱ'],
        'ひ' => &['び', 'ぴ'],
        'ふ' => &['ぶ', 'ぷ'],
        'へ' => &['べ', 'ぺ'],
        'ほ' => &['ぼ', 'ぽ'],
        _ => &[],
    }
}

/// Swaps the trailing conjugated word's lemma into the span — dictionaries key by citation form, never 中途半端な.
fn citation_form_subrange(subrange: &[Term]) -> Option<Vec<Term>> {
    let last = subrange.last()?;
    let conjugable = matches!(
        last.part_of_speech,
        POS::Verb | POS::SuruVerb | POS::Adjective | POS::AdjectivalNoun
    );
    if !conjugable || last.lemma_form.is_empty() || last.lemma_form == last.full_segment {
        return None;
    }
    let mut terms = subrange.to_vec();
    let last = terms.last_mut().expect("non-empty subrange");
    last.full_segment = last.lemma_form.clone();
    last.full_segment_reading = last.lemma_reading.clone();
    Some(terms)
}

/// Candidate readings for a compound: the plain component concat first, then —
/// one boundary at a time — the same reading with an interior component's first
/// kana voiced (土曜+日 → [どようひ, どようび, どようぴ]).
fn phrase_reading_candidates(subrange: &[Term]) -> Vec<String> {
    let base: String = subrange.iter().map(|t| t.full_segment_reading.as_str()).collect();
    let base_chars: Vec<char> = base.chars().collect();
    let mut candidates = vec![base];

    let mut offset = subrange[0].full_segment_reading.chars().count();
    for term in &subrange[1..] {
        let len = term.full_segment_reading.chars().count();
        if len > 0 {
            if let Some(&first) = base_chars.get(offset) {
                for &variant in rendaku_variants(first) {
                    let mut chars = base_chars.clone();
                    chars[offset] = variant;
                    candidates.push(chars.into_iter().collect());
                }
            }
        }
        offset += len;
    }
    candidates
}

pub fn init_vibrato(
    dict_type: &DictType,
    progress_callback: Option<Box<dyn Fn(String) + Send>>,
) -> Result<Tokenizer, YomineError> {
    let dict = load_dictionary(dict_type, progress_callback)?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}
