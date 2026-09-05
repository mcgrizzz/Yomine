//! Which word a spelling denotes *at a given reading*.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        Mutex,
    },
};

use vibrato::Tokenizer;

use super::token_models::UnidicToken;
use crate::core::utils::normalize_japanese_text;

// (spelling, reading, canonical form, source entry). These do not apply at other readings.
const SPELLING_ALIASES: &[(&str, &str, &str, &str)] = &[
    ("大人しい", "おとなしい", "大人しい", "JMdict 1414190"),
    ("温和しい", "おとなしい", "大人しい", "JMdict 1414190"),
    ("温柔しい", "おとなしい", "大人しい", "JMdict 1414190"),
    ("事", "こと", "事", "Kanjipedia 0002541200"),
    ("縡", "こと", "事", "Kanjipedia 0002541200"),
    ("行く", "いく", "行く", "JMdict 1578850"),
    ("往く", "いく", "行く", "JMdict 1578850"),
];

/// Enough paths to surface a spelling's minority readings; 甘い needs four to reach ウマイ.
const NBEST_PATHS: usize = 12;

/// 甘い is 甘い read あまい but 旨い read うまい, so candidate identity can never come from
/// the spelling alone. Resolves lazily and caches, because a candidate bucket is tiny and
/// tokenizing every dictionary headword up front is not.
pub struct CandidateLexemeResolver {
    /// `None` only in tests that exercise the non-lexical evidence path.
    tokenizer: Option<Arc<Tokenizer>>,
    cache: Mutex<HashMap<(String, String), Option<String>>>,
}

impl CandidateLexemeResolver {
    pub fn new(tokenizer: Arc<Tokenizer>) -> Self {
        Self { tokenizer: Some(tokenizer), cache: Mutex::new(HashMap::new()) }
    }

    #[cfg(test)]
    pub(crate) fn unresolving() -> Self {
        Self { tokenizer: None, cache: Mutex::new(HashMap::new()) }
    }

    /// The unique lexeme in the inspected N-best paths, or `None` when those paths
    /// offer no such reading or conflicting words. This bounded search does not
    /// prove that every possible analysis has been inspected.
    pub fn resolve(&self, spelling: &str, reading: &str) -> Option<String> {
        // Preserve the spelling that is analyzed: folding the cache key alone
        // would make whichever script was queried first decide future answers.
        let key = (spelling.to_string(), normalize_japanese_text(reading));
        // These reading-constrained spelling aliases are lexical dictionary
        // evidence missing from UniDic, not identities inferred from rank ties.
        // JMdict 1414190 lists all three spellings under おとなしい:
        // https://www.edrdg.org/jmwsgi/entr.py?svc=jmdict&q=1414190
        // 漢字ペディア defines 縡 read こと as 事:
        // https://www.kanjipedia.jp/kanji/0002541200
        // JMdict 1578850 lists 行く / 往く; 逝く is a separate restricted entry.
        // https://www.edrdg.org/jmwsgi/entr.py?svc=jmdict&q=1578850
        if let Some((_, _, canonical, _)) = SPELLING_ALIASES
            .iter()
            .find(|(form, reading, _, _)| *form == spelling && *reading == key.1)
        {
            return Some((*canonical).to_string());
        }
        if let Some(cached) = self.cache.lock().expect("resolver cache poisoned").get(&key) {
            return cached.clone();
        }
        let resolved = self.analyse(spelling, &key.1);
        self.cache.lock().expect("resolver cache poisoned").insert(key, resolved.clone());
        resolved
    }

    fn analyse(&self, spelling: &str, reading: &str) -> Option<String> {
        let mut worker = self.tokenizer.as_ref()?.new_worker();
        worker.reset_sentence(spelling);
        worker.tokenize_nbest(NBEST_PATHS);

        let mut lexemes: Vec<String> = Vec::new();
        for path in 0..worker.num_nbest_paths() {
            let Some(iter) = worker.nbest_token_iter(path) else {
                continue;
            };
            let tokens: Vec<UnidicToken> =
                iter.map(|t| UnidicToken::from_parts(t.surface(), t.feature(), 0..0)).collect();
            // A spelling that only parses as several words is not one lexeme.
            let [token] = tokens.as_slice() else {
                continue;
            };
            if normalize_japanese_text(&token.surface_hatsuon) != reading {
                continue;
            }
            if !lexemes.contains(&token.lexeme) {
                lexemes.push(token.lexeme.clone());
            }
        }

        match lexemes.len() {
            1 => lexemes.pop(),
            _ => None,
        }
    }
}

impl std::fmt::Debug for CandidateLexemeResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CandidateLexemeResolver")
    }
}

#[cfg(test)]
pub(crate) fn test_tokenizer() -> Option<Arc<Tokenizer>> {
    use crate::{
        dictionary::token_dictionary::DictType,
        segmentation::tokenizer::init_vibrato,
    };
    match init_vibrato(&DictType::Unidic, None) {
        Ok(tokenizer) => Some(Arc::new(tokenizer)),
        Err(error) => {
            assert!(
                std::env::var_os("YOMINE_REQUIRE_UNIDIC").is_none(),
                "required UniDic failed to load: {error}"
            );
            eprintln!("Skipping UniDic-dependent test: {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn resolver() -> Option<CandidateLexemeResolver> {
        test_tokenizer().map(CandidateLexemeResolver::new)
    }

    #[test]
    fn documented_aliases_require_the_documented_reading() {
        let r = CandidateLexemeResolver::unresolving();
        assert_eq!(r.resolve("温柔しい", "オトナシイ").as_deref(), Some("大人しい"));
        assert_eq!(r.resolve("縡", "こと").as_deref(), Some("事"));
        assert_eq!(r.resolve("縡", "さい"), None);
        assert_eq!(r.resolve("温柔しい", "べつのよみ"), None);
        assert_eq!(r.resolve("往く", "いく").as_deref(), Some("行く"));
        assert_eq!(r.resolve("逝く", "いく"), None);
        assert_eq!(r.resolve("逝く", "べつのよみ"), None);
    }

    #[test]
    fn the_reading_decides_which_word_a_spelling_is() {
        let Some(r) = resolver() else { return };

        assert_eq!(r.resolve("甘い", "うまい").as_deref(), Some("旨い"));
        assert_eq!(r.resolve("甘い", "あまい").as_deref(), Some("甘い"));
    }

    #[test]
    fn spellings_of_one_word_share_a_lexeme() {
        let Some(r) = resolver() else { return };

        for spelling in ["うまい", "上手い", "旨い"] {
            assert_eq!(r.resolve(spelling, "うまい").as_deref(), Some("旨い"), "{spelling}");
        }
    }

    #[test]
    fn an_ambiguous_or_absent_reading_resolves_to_nothing() {
        let Some(r) = resolver() else { return };

        // 橋 and 端 also parse as the ハシ given name, so neither answer is identity.
        assert_eq!(r.resolve("橋", "はし"), None);
        assert_eq!(r.resolve("端", "はし"), None);
        // UniDic has no answer for these spellings, but documented lexical
        // aliases now provide the missing evidence at this specific reading.
        assert_eq!(r.resolve("温柔しい", "おとなしい").as_deref(), Some("大人しい"));
        assert_eq!(r.resolve("温和しい", "おとなしい").as_deref(), Some("大人しい"));
    }

    #[test]
    fn distinct_homophones_keep_distinct_lexemes() {
        let Some(r) = resolver() else { return };

        assert_eq!(r.resolve("箸", "はし").as_deref(), Some("箸"));
        assert_ne!(r.resolve("箸", "はし"), r.resolve("甘い", "うまい"));
    }

    #[test]
    fn the_answer_is_cached_per_spelling_and_reading() {
        let Some(r) = resolver() else { return };

        assert_eq!(r.resolve("甘い", "うまい"), r.resolve("甘い", "うまい"));
        assert_ne!(r.resolve("甘い", "うまい"), r.resolve("甘い", "あまい"));
        assert_eq!(r.cache.lock().unwrap().len(), 2, "one entry per (spelling, reading)");
    }
}
