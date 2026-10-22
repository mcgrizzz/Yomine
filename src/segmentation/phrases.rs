//! Phrase promotion: which runs of a sentence's words become one term (二日酔い, やり方,
//! しょうがない), and why the others don't.

use super::{
    grammar,
    tokenizer::{
        phrase_candidate,
        phrase_endpoint_ok,
        PhraseMode,
    },
    unidic_tags::UnidicTag,
    word::{
        Word,
        POS,
    },
};
use crate::{
    core::{
        utils::is_kanji_char,
        Term,
    },
    dictionary::{
        frequency_manager::FrequencyManager,
        jmdict_lexicon,
    },
};

const MIN_CHARS: usize = 4;
/// Kanji noun compounds (複合体), JMdict words UniDic splits (居+場所) and JMdict phrases with
/// a kanji (何度も) can be this short.
const MIN_CHARS_WORD: usize = 3;
/// A JMdict word all in kanji (聖杖, split 聖 + 杖) can be this short.
const MIN_CHARS_KANJI_WORD: usize = 2;

/// Why a run of words isn't promoted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rejected {
    /// Starts or ends with a particle, and JMdict doesn't list it.
    Endpoint,
    /// No frequency list ranks it.
    Unranked,
    TooShort,
    /// お + a verb stem before ください or になる.
    PoliteVerb,
    /// お + a verb stem that starts a listed phrase (お迎えに来る is お + 迎えに来る).
    PhraseInside,
    /// Opens with a particle (につれて), yet its verb goes on into a subsidiary verb (連れていく).
    VerbChain,
    /// JMdict doesn't list it: frequency lists carry n-grams of words that only happen to sit
    /// together (どうする, 探し続ける, 戦闘経験).
    Unlisted,
    /// Opens with the そう of appearance (できそうです), which belongs to the word before it;
    /// JMdict's そうです and そうですね are the adverb そう.
    AppearanceSou,
}

pub(super) struct Phrase {
    pub term: Term,
    /// The words inside it stop being terms of their own.
    pub hides_parts: bool,
}

/// One sentence's words, before any phrase is added, and what is known about each.
pub(super) struct Context<'a> {
    pub terms: &'a [Term],
    pub facts: &'a [WordFacts],
    pub manager: &'a FrequencyManager,
}

/// What phrase promotion needs to know about a word, from its UniDic tokens.
pub(super) struct WordFacts {
    /// The type of a word ending in this one, where it isn't this word's own.
    ending_type: Option<POS>,
    grammatical: bool,
    rule_citation: bool,
    /// Ends in the conjunctive て/で (連れて).
    te_form: bool,
    /// A subsidiary verb or adjective (いく in 連れていく).
    subsidiary: bool,
    appearance_sou: bool,
}

impl WordFacts {
    pub(super) fn of(word: &Word) -> Self {
        let last = word.tokens.last();
        Self {
            ending_type: type_as_last_piece(word),
            grammatical: grammar::is_grammatical(word),
            rule_citation: word.has_rule_citation(),
            te_form: last.is_some_and(|t| {
                t.pos2 == UnidicTag::Setsuzokujoshi && matches!(t.surface.as_str(), "て" | "で")
            }),
            subsidiary: grammar::is_subsidiary(word),
            appearance_sou: word.tokens.first().is_some_and(|t| t.lexeme == "そう-様態"),
        }
    }
}

/// The phrase over `terms[start..=end]`, or why there is none.
pub(super) fn judge(ctx: &Context, start: usize, end: usize) -> Result<Phrase, Rejected> {
    let subrange = &ctx.terms[start..=end];
    let first = &subrange[0];
    let last = &subrange[end - start];
    // Frequency lists carry particle n-grams (あなたに); JMdict lists only real
    // phrases that begin or end in one (ついでに, にとって), and words (何でもない).
    let listed_particle_phrase = || {
        let text: String = subrange.iter().map(|t| t.full_segment.as_str()).collect();
        (subrange.iter().any(phrase_endpoint_ok) && jmdict_lexicon::is_phrase(&text))
            || jmdict_lexicon::is_word(&text)
    };
    if (!phrase_endpoint_ok(first) || !phrase_endpoint_ok(last)) && !listed_particle_phrase() {
        return Err(Rejected::Endpoint);
    }
    if ctx.facts[start].appearance_sou {
        return Err(Rejected::AppearanceSou);
    }
    let (mut phrase, frequency) = phrase_candidate(subrange, ctx.manager, PhraseMode::Production);
    if frequency.is_none() {
        return Err(Rejected::Unranked);
    }

    let all_nouns = subrange
        .iter()
        .all(|t| matches!(t.part_of_speech, POS::Noun | POS::CompoundNoun | POS::ProperNoun));
    let all_content_words = subrange.iter().all(phrase_endpoint_ok);
    let kanji_noun_compound = all_nouns && phrase.lemma_form.chars().all(is_kanji_char);
    // A JMdict phrase is no junk n-gram, and it reads as one unit even
    // around a particle (しょうがない).
    let listed_phrase = jmdict_lexicon::is_phrase(&phrase.surface_form)
        || jmdict_lexicon::is_phrase(&phrase.lemma_form);
    let jmdict_word = jmdict_lexicon::is_word(&phrase.surface_form)
        || jmdict_lexicon::is_word(&phrase.lemma_form);
    let word = !listed_phrase && jmdict_word;
    let listed = listed_phrase || word;

    // With a demonstrative or particle in them (あの人, 森の奥), three characters are two words.
    // The lexicon also lists adverbs as phrases (こたび).
    let short_word = jmdict_word && all_content_words && first.part_of_speech != POS::Determiner;
    // In kana, three characters spell some JMdict expression too often (それに, これは).
    let short_phrase = listed_phrase && phrase.surface_form.chars().any(is_kanji_char);
    let min_chars = if word && phrase.lemma_form.chars().all(is_kanji_char) {
        MIN_CHARS_KANJI_WORD
    } else if kanji_noun_compound || short_word || short_phrase {
        MIN_CHARS_WORD
    } else {
        MIN_CHARS
    };
    if phrase.lemma_form.chars().count() < min_chars {
        return Err(Rejected::TooShort);
    }

    // A phrase opening with a particle (につれて, について) is grammar that ends its clause.
    if first.part_of_speech == POS::Postposition
        && ctx.facts[end].te_form
        && ctx.facts.get(end + 1).is_some_and(|f| f.subsidiary)
    {
        return Err(Rejected::VerbChain);
    }
    // お + a verb stem before ください or になる is that verb said politely
    // (お通りください, お帰りになる), not the noun they spell.
    let after = |k: usize| ctx.terms.get(end + k);
    let lemma_after = |k: usize, lemmas: &[&str]| {
        after(k).is_some_and(|t| lemmas.contains(&t.lemma_form.as_str()))
    };
    let polite_verb = first.part_of_speech == POS::Prefix
        && last.part_of_speech == POS::Verb
        && (lemma_after(1, &["ください", "くださる", "下さる", "なさる"])
            || (after(1).is_some_and(|t| t.surface_form == "に")
                && lemma_after(2, &["なる", "成る"])));
    if polite_verb {
        return Err(Rejected::PoliteVerb);
    }
    let phrase_from_stem = || (end + 1..ctx.terms.len()).any(|e| judge(ctx, start + 1, e).is_ok());
    if first.part_of_speech == POS::Prefix && last.part_of_speech == POS::Verb && phrase_from_stem()
    {
        return Err(Rejected::PhraseInside);
    }
    if !listed {
        return Err(Rejected::Unlisted);
    }

    let jmdict_type = jmdict_lexicon::word_type(&phrase.lemma_form)
        .or_else(|| jmdict_lexicon::word_type(&phrase.surface_form));
    phrase.part_of_speech =
        phrase_type(all_nouns, word, last, &ctx.facts[end].ending_type, jmdict_type);
    // JMdict's verbs, adjectives and adverbs among them are vocabulary (ありえない, わけない).
    phrase.auto_skip.grammar = ctx.facts[start..=end].iter().all(|f| f.grammatical)
        && !matches!(phrase.part_of_speech, POS::Verb | POS::Adjective | POS::Adverb);
    phrase.frequencies =
        ctx.manager.build_freq_map(&phrase.lemma_form, &phrase.lemma_reading, phrase.is_kana);
    let hides_parts =
        (all_content_words || listed) && !ctx.facts[start..=end].iter().any(|f| f.rule_citation);
    Ok(Phrase { term: phrase, hides_parts })
}

/// JMdict's type where it gives one (本当に an adverb). Otherwise a word takes the type of its
/// last part (用心深い is an adjective), or the type that part forms (`type_as_last_piece`:
/// やり方 is a noun); anything else is an expression.
fn phrase_type(
    all_nouns: bool,
    word: bool,
    last: &Term,
    ending: &Option<POS>,
    jmdict: Option<POS>,
) -> POS {
    match (ending, jmdict) {
        _ if all_nouns => POS::NounExpression,
        // JMdict lists the noun of a suru verb (親孝行 in 親孝行して).
        (_, Some(jmdict)) if last.part_of_speech != POS::SuruVerb => jmdict,
        (Some(ending), _) if word => *ending,
        (None, _) if word && last.part_of_speech != POS::Suffix => last.part_of_speech,
        _ => POS::Expression,
    }
}

/// The type of a word that ends in `word`, where it isn't `word`'s own: a suffix's is the type
/// it forms (やり方 a noun, 嫌がる a verb), a bare verb stem names a noun (お迎え), and a
/// number ends a noun (朝イチ).
fn type_as_last_piece(word: &Word) -> Option<POS> {
    let first = word.tokens.first()?;
    if first.pos1 == UnidicTag::Setsubiji {
        return match first.pos2 {
            // UniDic files すぎ with the na-adjective suffixes, but 飲みすぎ is a noun.
            UnidicTag::Keijoushiteki if first.lexeme == "過ぎ" => Some(POS::Noun),
            UnidicTag::Meishiteki => Some(POS::Noun),
            UnidicTag::Doushiteki => Some(POS::Verb),
            UnidicTag::Keiyoushiteki => Some(POS::Adjective),
            UnidicTag::Keijoushiteki => Some(POS::AdjectivalNoun),
            _ => None,
        };
    }
    let bare_stem = word.tokens.len() == 1
        && first.pos1 == UnidicTag::Doushi
        && first.conjugation_form == UnidicTag::Renyoukei;
    (bare_stem || word.part_of_speech == POS::Number).then_some(POS::Noun)
}
