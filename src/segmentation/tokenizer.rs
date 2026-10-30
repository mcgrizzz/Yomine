use std::collections::{
    HashMap,
    HashSet,
};

use vibrato::{
    tokenizer::worker::Worker,
    Tokenizer,
};
use wana_kana::IsJapaneseStr;

use super::{
    nbest::rescue_words,
    phrases::{
        self,
        Rejected,
    },
    rule_matcher::parse_into_words,
    speaker_labels,
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
    // UniDic reads an i-adjective like つまらない as a verb and ない (つまる). Written in kana
    // it is the adjective; with kanji (行けない) it is usually the verb's negative.
    let kana = word.surface_form.as_str().is_kana();
    let exception = deinflections
        .iter()
        .find(|(form, reading)| kana && form == reading && jmdict_lexicon::is_nai_adjective(form))
        .cloned()
        .or_else(|| {
            // Speech shortens ない to ん (すまん, くだらん), which the deinflector doesn't undo.
            let form = format!("{}ない", word.surface_form.strip_suffix('ん')?);
            (kana && jmdict_lexicon::is_nai_adjective(&form)).then(|| (form.clone(), form))
        });
    // An honorific verb's imperative, the only one ending in い, is a word of its own rather
    // than a form of the verb (下さい "please", なさい, いらっしゃいませ).
    let surface_reading = fold_katakana(&word.surface_hatsuon);
    let honorific_imperative = surface_reading
        .strip_suffix("ませ")
        .unwrap_or(&surface_reading)
        .strip_suffix('い')
        .is_some_and(|stem| fold_katakana(&word.lemma_hatsuon).strip_suffix('る') == Some(stem))
        .then(|| (word.surface_form.clone(), surface_reading.clone()));
    let lemma = (word.lemma_form.clone(), word.lemma_hatsuon.clone());
    // UniDic's form wins when the deinflector agrees; otherwise it only competes on frequency,
    // so the deinflector's gaps (すん for する) don't lose it, but ください stays ください.
    // The deinflector writes katakana in hiragana (きれる for UniDic's キレる).
    let deinflected_lemma = deinflections.iter().any(|(form, reading)| {
        *reading == lemma.1 && fold_katakana(form) == fold_katakana(&lemma.0)
    });
    let lemma_deinflected = deinflections.contains(&lemma);
    // UniDic files a potential form under the verb it comes from: なれる (and なれん) under
    // 成る, read ナル. A compound's voiced first sound (づける for つける) isn't one.
    let plain = |reading: &str| {
        let mut reading = fold_katakana(reading);
        unvoice_first(&mut reading);
        reading
    };
    let potential_of = word
        .tokens
        .first()
        .filter(|t| {
            t.pos1 == UnidicTag::Doushi && plain(&t.lexeme_reading) != plain(&t.lemma_hatsuon)
        })
        .and_then(|t| {
            pairwise_deinflection(&lemma.0, &lemma.1).into_iter().find(|(form, reading)| {
                fold_katakana(reading) == fold_katakana(&t.lexeme_reading)
                    && manager.get_harmonic_frequency_for_pair(form, reading).is_some()
            })
        });
    let mut candidates: Vec<_> = deinflections
        .into_iter()
        .chain((!lemma_deinflected).then(|| lemma.clone()))
        .filter(|(form, reading)| manager.get_harmonic_frequency_for_pair(form, reading).is_some())
        .collect();
    candidates
        .sort_by_key(|(form, reading)| manager.get_harmonic_frequency_for_pair(form, reading));
    match word.part_of_speech {
        POS::Verb => retain_verb_final_candidates(&mut candidates, &word.surface_form),
        // An adjective stem can deinflect as a verb (のろ as 乗る's imperative, 好き as 好く).
        POS::Adjective | POS::AdjectivalNoun => {
            candidates.retain(|pair| pair.0.ends_with('い') || *pair == lemma)
        }
        _ => {}
    }
    let nai_adjective = exception.is_some();
    let selected = exception
        .or(honorific_imperative)
        .map(|pair| (pair, CitationProvenance::LexicalException))
        .or_else(|| {
            potential_of
                .as_ref()
                .or_else(|| candidates.iter().find(|pair| deinflected_lemma && **pair == lemma))
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
        if nai_adjective {
            word.part_of_speech = POS::Adjective;
        }
        word.lemma_form = form.clone();
        word.lemma_hatsuon = reading.clone();
        word.citation = Some(Citation { form, reading, provenance });
    }
}

fn analyze_sentence(worker: &mut Worker, text: &str, manager: &FrequencyManager) -> Vec<Word> {
    worker.reset_sentence(text);
    worker.tokenize();
    let mut tokens: Vec<UnidicToken> = worker
        .token_iter()
        .map(|t| UnidicToken::from_parts(t.surface(), t.feature(), t.range_byte()))
        .collect();
    if misparsed(&tokens) {
        worker.tokenize_nbest(MISPARSE_PATHS);
        let better = (1..worker.num_nbest_paths())
            .filter_map(|p| {
                let path = worker.nbest_token_iter(p)?;
                Some(
                    path.map(|t| UnidicToken::from_parts(t.surface(), t.feature(), t.range_byte()))
                        .collect::<Vec<_>>(),
                )
            })
            .find(|path| !misparsed(path));
        if let Some(better) = better {
            tokens = better;
        }
    }
    read_verbs_as_commoner(worker, &mut tokens, manager);
    read_one_day(&mut tokens);
    join_listed_sahen_compounds(&mut tokens);
    let mut words = parse_into_words(tokens).unwrap_or_default();
    unvoice_initial_sounds(&mut words);
    let words = reparse_katakana_speech(worker, words, manager);
    let mut words = split_unlisted_affixes(
        split_unvalidated_compounds(rescue_words(worker, text, words, manager), manager),
        manager,
    );
    keep_listed_words(&mut words, manager);
    keep_listed_suru_verbs(&mut words);
    tag_katakana_interjections(worker, &mut words);
    read_contracted_prohibitive(&mut words);
    tag_split_syllables(&mut words);
    words
}

/// Voicing comes from the part before it (中央 + どおり, 思い + どおり), so a word after
/// anything but a noun, affix or verb stem has its plain first sound; UniDic sometimes keeps
/// the voiced reading it uses inside compounds (掛ける as がける after として). The lexeme's
/// reading has the plain sound.
fn unvoice_initial_sounds(words: &mut [Word]) {
    for i in 0..words.len() {
        let after_compound_part = i > 0
            && words[i - 1].tokens.last().is_some_and(|t| {
                matches!(t.pos1, UnidicTag::Meishi | UnidicTag::Settouji | UnidicTag::Setsubiji)
                    || (t.pos1 == UnidicTag::Doushi && t.conjugation_form == UnidicTag::Renyoukei)
            });
        let word = &mut words[i];
        let Some(first) = word.tokens.first_mut() else {
            continue;
        };
        // Kana spells a voiced sound out (どころ, ぐらい); only a kanji can hide one.
        let written_in_kanji = first.surface.chars().next().is_some_and(is_kanji_char)
            && matches!(first.pos1, UnidicTag::Meishi | UnidicTag::Doushi | UnidicTag::Keiyoushi);
        if after_compound_part || !written_in_kanji {
            continue;
        }
        let voiced = first.lemma_hatsuon.chars().next().and_then(plain_sound);
        if voiced.is_none() || voiced != first.lexeme_reading.chars().next() {
            continue;
        }
        let start = first.start_byte;
        unvoice_first(&mut first.surface_hatsuon);
        unvoice_first(&mut first.lemma_hatsuon);
        unvoice_first(&mut word.surface_hatsuon);
        unvoice_first(&mut word.lemma_hatsuon);
        if let Some(main) = word.main_word.as_mut().filter(|m| m.start_byte == start) {
            unvoice_first(&mut main.surface_hatsuon);
            unvoice_first(&mut main.lemma_hatsuon);
        }
    }
}

/// The plain sound of a voiced kana in either script (が → か, ぱ → は).
fn plain_sound(kana: char) -> Option<char> {
    const VOICED: &str = "ガギグゲゴザジズゼゾダヂヅデドバビブベボパピプペポ";
    let katakana = match kana {
        'ぁ'..='ゖ' => char::from_u32(kana as u32 + 0x60)?,
        _ => kana,
    };
    let offset = match VOICED.find(katakana)? / 'ガ'.len_utf8() {
        index if index >= 20 => 2,
        _ => 1,
    };
    char::from_u32(kana as u32 - offset)
}

fn unvoice_first(reading: &mut String) {
    let mut chars = reading.chars();
    if let Some(plain) = chars.next().and_then(plain_sound) {
        *reading = std::iter::once(plain).chain(chars).collect();
    }
}

/// No word starts with a small kana or ー; UniDic leaves such a run when it splits a
/// syllable it can't read (しなけれ + ゃいけないんですから).
fn tag_split_syllables(words: &mut [Word]) {
    const SPLIT_STARTS: &[char] = &[
        'ゃ', 'ゅ', 'ょ', 'ぁ', 'ぃ', 'ぅ', 'ぇ', 'ぉ', 'ゎ', 'ャ', 'ュ', 'ョ', 'ァ', 'ィ', 'ゥ',
        'ェ', 'ォ', 'ー',
    ];
    for word in words {
        if word.surface_form.starts_with(SPLIT_STARTS)
            && matches!(
                word.part_of_speech,
                POS::Noun | POS::Verb | POS::Adjective | POS::AdjectivalNoun | POS::Adverb
            )
        {
            word.part_of_speech = POS::Unknown;
        }
    }
}

/// UniDic reads んな as そんな wherever it appears, but right after a verb it is るな with る
/// contracted (言わせんな, 来んな): the prohibitive particle な.
fn read_contracted_prohibitive(words: &mut [Word]) {
    for i in 1..words.len() {
        let after_verb = words[i - 1]
            .tokens
            .last()
            .is_some_and(|t| matches!(t.pos1, UnidicTag::Doushi | UnidicTag::Jodoushi));
        let word = &mut words[i];
        let [token] = word.tokens.as_slice() else {
            continue;
        };
        if after_verb && token.surface == "んな" && token.lexeme == "そんな" {
            word.part_of_speech = POS::Postposition;
            word.lemma_form = "な".into();
            word.lemma_hatsuon = "な".into();
            word.citation = None;
        }
    }
}

/// UniDic often reads 一日 as ついたち, the 1st, which it is only after a month (四月一日);
/// elsewhere it is いちにち (今日一日).
fn read_one_day(tokens: &mut [UnidicToken]) {
    for i in 0..tokens.len() {
        let after_month = i > 0 && tokens[i - 1].surface == "月";
        let token = &mut tokens[i];
        if token.surface == "一日" && token.surface_hatsuon == "ツイタチ" && !after_month {
            token.surface_hatsuon = "イチニチ".into();
            token.lemma_hatsuon = "イチニチ".into();
        }
    }
}

/// UniDic splits off a suru-able suffix (自動 + 化) even where JMdict lists the compound
/// (自動化); joined, a following する makes a suru verb of the compound instead of 化する.
fn join_listed_sahen_compounds(tokens: &mut Vec<UnidicToken>) {
    let mut i = 1;
    while i + 1 < tokens.len() {
        let (noun, suffix) = (&tokens[i - 1], &tokens[i]);
        let joined = format!("{}{}", noun.surface, suffix.surface);
        if noun.pos1 == UnidicTag::Meishi
            && suffix.pos1 == UnidicTag::Setsubiji
            && suffix.pos3 == UnidicTag::Sahenkanou
            && tokens[i + 1].conjugation_type == UnidicTag::Sagyouhenkaku
            && jmdict_lexicon::is_word(&joined)
        {
            let suffix = tokens.remove(i);
            let noun = &mut tokens[i - 1];
            noun.surface = joined;
            noun.pos3 = UnidicTag::Sahenkanou;
            noun.surface_hatsuon.push_str(&suffix.surface_hatsuon);
            noun.lemma_form.push_str(&suffix.lemma_form);
            noun.lemma_hatsuon.push_str(&suffix.lemma_hatsuon);
            noun.lexeme.push_str(&suffix.lexeme);
            noun.lexeme_reading.push_str(&suffix.lexeme_reading);
            noun.end_byte = suffix.end_byte;
        }
        i += 1;
    }
}

/// A noun + する that JMdict lists as a verb of its own (値する, "to deserve") means more than
/// the noun, so the verb is the word. A noun right after a verb stem ends a compound (忘れ物
/// in 忘れ物した), not 物する.
fn keep_listed_suru_verbs(words: &mut [Word]) {
    for i in 0..words.len() {
        let after_verb_stem = i > 0
            && words[i - 1].tokens.last().is_some_and(|t| {
                t.pos1 == UnidicTag::Doushi && t.conjugation_form == UnidicTag::Renyoukei
            });
        let word = &mut words[i];
        if word.part_of_speech == POS::SuruVerb
            && !after_verb_stem
            && jmdict_lexicon::word_type(&word.lemma_form) == Some(POS::Verb)
        {
            word.main_word = None;
            word.part_of_speech = POS::Verb;
        }
    }
}

const MISPARSE_PATHS: usize = 5;

/// How much more UniDic's model may charge for a parse and still have frequency overrule it:
/// 勝って costs 572 more as 勝つ than as 勝る in 全部をベットしてでも勝ってやる, while 通って
/// costs 1867 more as 通る than as the right 通う in 毎日通ってくださっていた.
const NEAR_TIE_COST: i32 = 1500;

/// A verb UniDic reads as a rarer word written the same way when a parse it nearly prefers
/// reads it as the commoner one (勝って as 勝る, not 勝つ).
fn read_verbs_as_commoner(
    worker: &mut Worker,
    tokens: &mut [UnidicToken],
    manager: &FrequencyManager,
) {
    let kanji_verb =
        |t: &UnidicToken| t.pos1 == UnidicTag::Doushi && t.surface.chars().any(is_kanji_char);
    if !tokens.iter().any(kanji_verb) {
        return;
    }
    // Either spelling: dictionaries rank おっしゃる in kana, which makes 仰る look rare.
    let rank = |t: &UnidicToken| {
        let reading = fold_katakana(&t.lemma_hatsuon);
        [t.lemma_form.as_str(), reading.as_str()]
            .iter()
            .filter_map(|form| manager.get_harmonic_frequency_for_pair(form, &reading))
            .min()
            .unwrap_or(u32::MAX)
    };
    worker.tokenize_nbest(MISPARSE_PATHS);
    let Some(best) = worker.path_cost(0) else {
        return;
    };
    // Verb tokens of the near-tied paths, with UniDic's conjugation form as written: the stored
    // one leaves most forms unmapped, and 浮かべ is both 浮かべる's stem and 浮かぶ's conditional.
    let readings: Vec<(UnidicToken, String)> = (0..worker.num_nbest_paths())
        .filter(|&p| worker.path_cost(p).is_some_and(|cost| cost - best <= NEAR_TIE_COST))
        .filter_map(|p| worker.nbest_token_iter(p))
        .flatten()
        .filter_map(|t| {
            let form = t.feature().split(',').nth(5)?.to_string();
            Some((UnidicToken::from_parts(t.surface(), t.feature(), t.range_byte()), form))
        })
        .filter(|(t, _)| kanji_verb(t))
        .collect();
    let same_span =
        |a: &UnidicToken, b: &UnidicToken| (a.start_byte, a.end_byte) == (b.start_byte, b.end_byte);
    for token in tokens.iter_mut().filter(|t| kanji_verb(t)) {
        let Some((_, form)) =
            readings.iter().find(|(t, _)| same_span(t, token) && t.lemma_form == token.lemma_form)
        else {
            continue;
        };
        let commoner = readings
            .iter()
            .filter(|(alt, alt_form)| {
                same_span(alt, token) && alt_form == form && alt.lemma_form != token.lemma_form
            })
            .map(|(alt, _)| alt)
            .min_by_key(|alt| rank(alt))
            .filter(|alt| rank(alt) < rank(token));
        if let Some(commoner) = commoner {
            *token = commoner.clone();
        }
    }
}

/// Token runs the 1-best path gets wrong often enough that another path is worth taking.
fn misparsed(tokens: &[UnidicToken]) -> bool {
    final_particle_mid_clause(tokens) || noun_after_verb_stem(tokens)
}

/// A sentence-final particle ends its clause, so one followed by が, を or も is a misparse
/// (か + い for 甲斐 in 待ったかいがあった). A quote can end in one (行くかと思った).
fn final_particle_mid_clause(tokens: &[UnidicToken]) -> bool {
    tokens.windows(2).any(|w| {
        w[0].pos2 == UnidicTag::Shuujoshi
            && w[1].pos1 == UnidicTag::Joshi
            && matches!(w[1].surface.as_str(), "が" | "を" | "も")
    })
}

/// A verb stem joins a following noun only inside a compound, which UniDic keeps as one token,
/// so a kana noun right after one took the verb's ending (し + たんだい for した + んだい).
fn noun_after_verb_stem(tokens: &[UnidicToken]) -> bool {
    tokens.windows(2).any(|w| {
        matches!(w[0].pos1, UnidicTag::Doushi | UnidicTag::Jodoushi)
            && w[0].conjugation_form == UnidicTag::Renyoukei
            && w[1].pos1 == UnidicTag::Meishi
            && w[1].pos2 == UnidicTag::Futsuumeishi
            && w[1].surface.as_str().is_kana()
    })
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

/// The honorific and counter rules mine the part after the first (嬢 in お嬢様, 度 in
/// 一度). A word JMdict lists whole is a word of its own (一体, "what on earth"), so it
/// stays whole. It keeps UniDic's reading when a frequency list ranks it (一日 as いちにち
/// or ついたち, by context), else takes the one the lists mostly give: UniDic reads 兄 in
/// お兄さん as あに.
fn keep_listed_words(words: &mut [Word], manager: &FrequencyManager) {
    for word in words {
        let (Some(first), Some(main)) = (word.tokens.first(), word.main_word.as_ref()) else {
            continue;
        };
        if main.start_byte == first.start_byte {
            continue;
        }
        // The lexicon holds kana spellings only for words usually written in kana, so a kana
        // word (ごちそう) is left to the frequency lists.
        if !word.surface_form.as_str().is_kana() && !jmdict_lexicon::is_word(&word.surface_form) {
            continue;
        }
        let reading = phrase_frequency(manager, &word.surface_form, &word.surface_hatsuon)
            .map(|_| word.surface_hatsuon.clone())
            .or_else(|| {
                manager
                    .majority_reading(&word.surface_form)
                    .map(|reading| as_written(&word.surface_hatsuon, reading))
            });
        if let Some(reading) = reading {
            word.main_word = None;
            word.lemma_form = word.surface_form.clone();
            word.surface_hatsuon = reading.clone();
            word.lemma_hatsuon = reading;
            if word.part_of_speech == POS::Counter {
                word.part_of_speech = POS::Noun;
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum PhraseMode {
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

fn best_ranked(
    manager: &FrequencyManager,
    form: &str,
    readings: Vec<String>,
) -> Option<(u32, String)> {
    readings
        .into_iter()
        .filter_map(|reading| Some((phrase_frequency(manager, form, &reading)?, reading)))
        .min_by_key(|(rank, _)| *rank)
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
pub(super) fn phrase_candidate(
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
    // JMdict can list a compound under both readings (いつもどおり and いつもとおり); the
    // commoner one is meant.
    if let Some((rank, reading)) = best_ranked(manager, &phrase.surface_form, readings) {
        phrase.surface_reading = reading.clone();
        phrase.lemma_reading = reading.clone();
        phrase.full_segment_reading = reading;
        frequency = Some(rank);
    }
    // UniDic can read a component differently inside a compound (表 as ひょう, but
    // 表沙汰 is おもてざた; 何 as なに, but 何でもない is なんでもない), so a spelling every
    // dictionary reads one way takes that reading.
    if frequency.is_none()
        && matches!(mode, PhraseMode::Production)
        && jmdict_lexicon::is_word(&phrase.surface_form)
    {
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
    for citation in citation_form_subranges(subrange) {
        if frequency.is_some() {
            break;
        }
        let form: String = citation.iter().map(|t| t.full_segment.as_str()).collect();
        if let Some((rank, reading)) = best_ranked(manager, &form, mode.readings(&citation)) {
            phrase.lemma_form = form;
            phrase.lemma_reading = reading;
            frequency = Some(rank);
        }
    }
    (phrase, frequency)
}

pub fn extract_words(
    worker: Worker,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
) -> Vec<Term> {
    extract(worker, sentences, frequency_manager, None)
}

/// Every phrase candidate that a frequency list ranks, as (sentence index, its words,
/// verdict), for `examples/review.rs --why`.
pub fn explain_phrases(
    worker: Worker,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
) -> Vec<(usize, String, String)> {
    let mut verdicts = Vec::new();
    let mut record = |ord: usize, text: String, verdict: Result<(), Rejected>| {
        let verdict = match verdict {
            Ok(()) => "✓".to_string(),
            Err(Rejected::Endpoint | Rejected::Unranked) => return,
            Err(rejected) => format!("✗ {rejected:?}"),
        };
        verdicts.push((ord, text, verdict));
    };
    extract(worker, sentences, frequency_manager, Some(&mut record));
    verdicts
}

type PhraseTrace<'a> = &'a mut dyn FnMut(usize, String, Result<(), Rejected>);

fn extract(
    mut worker: Worker,
    sentences: &mut [Sentence],
    frequency_manager: &FrequencyManager,
    mut trace: Option<PhraseTrace>,
) -> Vec<Term> {
    let mut terms = Vec::<Term>::new();
    let mut speaker_names = HashSet::new();
    // Per term: occurrences tagged grammar, and all occurrences.
    let mut grammar_counts = HashMap::<(String, String), (usize, usize)>::new();

    for (ord, sentence) in sentences.iter_mut().enumerate() {
        let words = analyze_sentence(&mut worker, &sentence.text, frequency_manager);

        let mut term_spans: Vec<(usize, usize)> = Vec::with_capacity(words.len());
        let mut sentence_terms: Vec<Term> = Vec::with_capacity(words.len());
        let mut facts = Vec::with_capacity(words.len());
        let mut after_stem = false;
        for word in words {
            let span = word.byte_span();
            facts.push(phrases::WordFacts::of(&word));
            // A bound verb right after a verb stem adds grammar to it (覚えきれる, 食べすぎる).
            let bound_to_stem = after_stem
                && word.tokens.first().is_some_and(|t| {
                    t.pos1 == UnidicTag::Doushi && t.pos2 == UnidicTag::Hijiritsukanou
                });
            after_stem = word.tokens.last().is_some_and(|t| {
                t.pos1 == UnidicTag::Doushi && t.conjugation_form == UnidicTag::Renyoukei
            });
            let subsidiary_after_te = match facts.as_slice() {
                [.., te, link, this] if link.te_link => te.te_form && this.subsidiary,
                [.., te, this] => te.te_form && this.subsidiary,
                _ => false,
            };
            // A prefix or suffix alone only adds to the word beside it (たち, さん, 全).
            let lone_affix = matches!(
                word.tokens.as_slice(),
                [t] if matches!(t.pos1, UnidicTag::Settouji | UnidicTag::Setsubiji)
            );
            // The highlight span ends at start + surface_form.len(), so the
            // reference must point at the main word, not the segment.
            let ref_start = word.mining_span().0;
            let mut term: Term = word.into();
            term.auto_skip.grammar = bound_to_stem || subsidiary_after_te || lone_affix;
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
        let labels: Vec<(usize, usize)> = speaker_labels::bracket_labels(&sentence.text)
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
                let name = &sentence.text[start..end];
                let names = speaker_labels::names_speaker(name, &words, frequency_manager);
                if names {
                    speaker_names.insert(name.to_string());
                }
                names
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
        let ctx =
            phrases::Context { terms: &sentence_terms, facts: &facts, manager: frequency_manager };
        let mut promoted = Vec::new();
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
                let verdict = phrases::judge(&ctx, start, end);
                if let Some(trace) = trace.as_mut() {
                    let text = ctx.terms[start..=end].iter().map(|t| t.full_segment.as_str());
                    trace(ord, text.collect(), verdict.as_ref().map(|_| ()).map_err(|r| *r));
                }
                let Ok(mut phrase) = verdict else { continue };
                phrase.term.sentence_references.push((ord, term_spans[start].0));
                promoted.push(phrase.term);
                if phrase.hides_parts {
                    for flag in suppressed[start..=end].iter_mut() {
                        *flag = true;
                    }
                    hidden_through = Some(end);
                }
                // Largest phrase at this start position is accepted; move to next start.
                break;
            }
        }
        sentence_terms.extend(promoted);

        let mut idx = 0;
        sentence_terms.retain(|_| {
            let keep = idx >= base_len || !suppressed[idx];
            idx += 1;
            keep
        });

        for sentence_term in &sentence_terms {
            let key = (sentence_term.lemma_form.clone(), sentence_term.lemma_reading.clone());
            let (grammar, all) = grammar_counts.entry(key).or_default();
            *grammar += usize::from(sentence_term.auto_skip.grammar);
            *all += 1;
        }

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

    // Grammar where most occurrences are: 来る after a て-form is still a verb to learn where it
    // mostly comes on its own, and さん misread as 三 now and then stays an honorific.
    for term in &mut terms {
        let (grammar, all) = grammar_counts[&(term.lemma_form.clone(), term.lemma_reading.clone())];
        term.auto_skip.grammar = grammar * 2 > all;
    }

    speaker_labels::mark_speaker_names(&mut terms, sentences, &speaker_names);
    mark_kanji_fragments(&mut terms, sentences);

    terms
}

/// A single kanji written against another kanji wherever it occurs is part of a name or of a
/// compound no dictionary lists (氏 of 壬氏, 梅 of 梅梅, 商 of 交易商).
fn mark_kanji_fragments(terms: &mut [Term], sentences: &[Sentence]) {
    for term in terms {
        let mut chars = term.surface_form.chars();
        let (Some(kanji), None) = (chars.next(), chars.next()) else {
            continue;
        };
        if !is_kanji_char(kanji) {
            continue;
        }
        term.auto_skip.kanji_fragment = !term.sentence_references.is_empty()
            && term.sentence_references.iter().all(|&(ord, start)| {
                let text = &sentences[ord].text;
                text[start..].starts_with(kanji)
                    && (text[..start].chars().next_back().is_some_and(is_kanji_char)
                        || text[start + kanji.len_utf8()..]
                            .chars()
                            .next()
                            .is_some_and(is_kanji_char))
            });
    }
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

pub(super) fn phrase_endpoint_ok(term: &Term) -> bool {
    !matches!(term.part_of_speech, POS::Postposition | POS::Copula | POS::Symbol)
}

/// Split rule-merged proper+common compounds back apart (夏子おばさん →
/// 夏子 + おばさん) unless JMdict lists the compound or a frequency dictionary corroborates
/// the merged pair. A listed compound takes a JMdict reading when UniDic's pieces don't give
/// one (甘納豆 as あまなっとう, not かんなっとう).
fn split_unvalidated_compounds(
    words: Vec<Word>,
    frequency_manager: &FrequencyManager,
) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::with_capacity(words.len());
    for mut word in words {
        if word.part_of_speech != POS::CompoundNoun || word.tokens.len() < 2 {
            out.push(word);
            continue;
        }
        let listed = jmdict_lexicon::readings(&word.surface_form);
        if !listed.is_empty() {
            let plain = |reading: &str| fold_katakana(reading).normalize_long_vowel().into_owned();
            let reading =
                if listed.iter().any(|reading| plain(reading) == plain(&word.surface_hatsuon)) {
                    // Each piece in its own script, as JMdict writes them (マルクスけいざいがく).
                    word.tokens
                        .iter()
                        .map(|token| normalize_reading(&token.surface, &token.surface_hatsuon))
                        .collect()
                } else {
                    let readings = listed.iter().map(|reading| reading.to_string()).collect();
                    best_ranked(frequency_manager, &word.surface_form, readings)
                        .map_or_else(|| listed[0].to_string(), |(_, reading)| reading)
                };
            word.lemma_hatsuon = reading.clone();
            word.surface_hatsuon = reading;
            out.push(word);
            continue;
        }
        if frequency_manager.get_enabled_dictionaries().is_empty()
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

/// A noun with a prefix or suffix JMdict doesn't list as one word (者たち, 全マップ,
/// 寝室らしき) is the noun plus its affixes, so the card is the noun. Counters stay on their
/// numbers (五枚). The lexicon lists few kana spellings, so a kana word a frequency list ranks
/// stays whole (わたしたち).
fn split_unlisted_affixes(words: Vec<Word>, manager: &FrequencyManager) -> Vec<Word> {
    let mut out = Vec::with_capacity(words.len());
    for word in words {
        let tokens = &word.tokens;
        // UniDic cuts unknown katakana names at a katakana affix (フラン + メ).
        let affix = |t: &UnidicToken, tag| t.pos1 == tag && !t.surface.as_str().is_katakana();
        let prefixes = tokens.iter().take_while(|t| affix(t, UnidicTag::Settouji)).count();
        let suffixes = tokens[prefixes..]
            .iter()
            .rev()
            .take_while(|t| affix(t, UnidicTag::Setsubiji) && t.pos3 != UnidicTag::Josuushi)
            .count();
        let core = &tokens[prefixes..tokens.len() - suffixes];
        let nominal = |t: &UnidicToken| {
            (t.pos1 == UnidicTag::Meishi && t.pos2 != UnidicTag::Suushi)
                || t.pos1 == UnidicTag::Daimeshi
        };
        if prefixes + suffixes == 0
            || core.is_empty()
            || !core.iter().all(nominal)
            || word.main_word.is_some()
            || jmdict_lexicon::is_word(&word.surface_form)
            || jmdict_lexicon::is_word(&word.lemma_form)
            || (word.surface_form.as_str().is_kana()
                && manager
                    .get_harmonic_frequency_for_pair(&word.surface_form, &word.surface_hatsuon)
                    .is_some())
        {
            out.push(word);
            continue;
        }
        let parts = [&tokens[..prefixes], core, &tokens[tokens.len() - suffixes..]];
        let split: Result<Vec<Vec<Word>>, _> = parts
            .iter()
            .filter(|part| !part.is_empty())
            .map(|part| parse_into_words(part.to_vec()))
            .collect();
        match split {
            Ok(mut parts) => {
                if let Some(noun) = parts[1].first_mut().filter(|_| prefixes > 0) {
                    drop_prefix_voicing(noun);
                }
                out.extend(parts.into_iter().flatten());
            }
            Err(_) => out.push(word),
        }
    }
    out
}

/// A noun cut from its prefix loses the voicing the prefix gave it (大ガエル → カエル, whose
/// lexeme reading has the plain sound). A kana spelling shows the voicing, so the card's
/// spelling loses it too; the surface stays as written.
fn drop_prefix_voicing(word: &mut Word) {
    let Some(first) = word.tokens.first_mut() else {
        return;
    };
    let voiced = first.lemma_hatsuon.chars().next().and_then(plain_sound);
    if voiced.is_none() || voiced != first.lexeme_reading.chars().next() {
        return;
    }
    unvoice_first(&mut first.surface_hatsuon);
    unvoice_first(&mut first.lemma_hatsuon);
    unvoice_first(&mut word.surface_hatsuon);
    unvoice_first(&mut word.lemma_hatsuon);
    unvoice_first(&mut word.lemma_form);
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
/// Its lemma comes first, then the forms derived from that lemma it could also be: a phrase
/// can use a potential or causative verb (引ける in 気が引ける, 輝かせる in 目を輝かせる).
fn citation_form_subranges(subrange: &[Term]) -> Vec<Vec<Term>> {
    let Some(last) = subrange.last() else {
        return Vec::new();
    };
    let conjugable = matches!(
        last.part_of_speech,
        POS::Verb | POS::SuruVerb | POS::Adjective | POS::AdjectivalNoun
    );
    if !conjugable || last.lemma_form.is_empty() || last.lemma_form == last.full_segment {
        return Vec::new();
    }
    let mut forms = vec![(last.lemma_form.clone(), last.lemma_reading.clone())];
    for pair in pairwise_deinflection(&last.full_segment, &last.full_segment_reading) {
        let derived = pairwise_deinflection(&pair.0, &pair.1)
            .iter()
            .any(|(form, _)| *form == last.lemma_form);
        if derived && !forms.contains(&pair) {
            forms.push(pair);
        }
    }
    forms
        .into_iter()
        .map(|(form, reading)| {
            let mut terms = subrange.to_vec();
            let last = terms.last_mut().expect("non-empty subrange");
            last.full_segment = form;
            last.full_segment_reading = reading;
            terms
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_mode_skips_kanji_fragments_and_bound_verbs() {
        let Some(tokenizer) = crate::segmentation::lexeme_resolver::test_tokenizer() else {
            return;
        };
        let manager = FrequencyManager::from_dictionaries(vec![]);
        let mut sentences: Vec<Sentence> = [
            "梅梅姐ちゃんの感情",
            "白い蕾に吸わせたんです",
            "果たして 覚えきれるかどうか",
            "横になって下さい",
            "本をください",
            "家に誰かいる",
            "待っている",
            "見ている",
            "売ってもくれない",
        ]
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
        let skip = |surface: &str| {
            terms.iter().find(|t| t.surface_form == surface).map(|t| t.auto_skip.clone())
        };
        assert_eq!(skip("梅").map(|s| s.kanji_fragment), Some(true));
        assert_eq!(skip("蕾").map(|s| s.kanji_fragment), Some(false));
        assert_eq!(skip("きれる").map(|s| s.grammar), Some(true));
        assert_eq!(skip("覚え").map(|s| s.grammar), Some(false));
        assert_eq!(skip("下さい").map(|s| s.grammar), Some(true));
        assert_eq!(skip("ください").map(|s| s.grammar), Some(false));
        assert_eq!(skip("いる").map(|s| s.grammar), Some(true));
        assert_eq!(skip("くれない").map(|s| s.grammar), Some(true));
    }
}
