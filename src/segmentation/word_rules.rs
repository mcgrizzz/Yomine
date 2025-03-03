use super::rule_matcher::{Rule, TokenMatcher, RuleAction, MainWordPolicy};
use super::word::POS;
use super::unidic_tags::UnidicTag;

/**
 * These rules are not necessarily how an online parser would parse like ichi.moe. We want to prioritize minable terms and what we will be adding and checking against in our dictionary
 * Ex: suru verbs will be kept separate, adjectival nouns are left without na 
 * To decide:
 * //Do we keep Adjectival noun + ni together such as shizuka ni, this can be pretty common 
 * */
pub fn create_default_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "Join numbers",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Meishi),
                pos2: Some(UnidicTag::Suushi),
                ..Default::default()
            },
            next: None,
            prev: None,
            prev_word_pos: Some(POS::Number),
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "Na-adj + na",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Keijoushi),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Some(UnidicTag::Jodoushi),
                conjugation_type: Some(UnidicTag::JodoushiDa),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::Adjective,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },

        Rule {
            name: "Na-adj possible + na",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Meishi),
                pos3: Some(UnidicTag::Keijoushikanou),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Some(UnidicTag::Jodoushi),
                surface: Some("な".to_string()),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::Adjective,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },

        Rule {
            name: "suru-possible + suru",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Meishi),
                pos3: Some(UnidicTag::Sahenkanou),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                conjugation_type: Some(UnidicTag::Sagyouhenkaku),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::Verb,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },
        Rule {
            name: "Jodoushi to jodoushi binding",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Jodoushi),
                conjugation_type_fn: Some(|tag| {
                    matches!(
                        *tag,
                        UnidicTag::JodoushiTa | 
                        UnidicTag::JodoushiNai |
                        UnidicTag::JodoushiTai |
                        UnidicTag::JodoushiMasu |
                        UnidicTag::JodoushiNu
                    )
                }),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Some(UnidicTag::Jodoushi),
                ..Default::default()
            }),
            prev_word_pos: None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },
        
        Rule {
            name: "Verb masu",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Jodoushi),
                conjugation_type: Some(UnidicTag::JodoushiMasu),
                ..Default::default()
            },
            next: None,
            prev:  Some(TokenMatcher {
                pos1: Some(UnidicTag::Doushi),
                ..Default::default()
            }),
            prev_word_pos: Some(POS::Verb),
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },

        Rule {
            name: "Suffix to noun",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Setsubiji),
                ..Default::default()
            },
            next: None,
            prev: None,
            prev_word_pos: Some(POS::Noun),
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None, //We can get some weird stuff with combining ex: おばあちゃん家 will have ばあ as the main word which is not great. In these rare cases, the full word should have low or no frequency and be sunk to the bottom anyway.
            },
        },

        Rule {
            name: "Prefix Noun",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Settouji),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Some(UnidicTag::Meishi),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::Noun,
                main_word_policy: Some(MainWordPolicy::UseSecondToken),
            },
        },
        Rule {
            name: "Prefix Verb",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Settouji),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Some(UnidicTag::Doushi),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::Verb,
                main_word_policy: Some(MainWordPolicy::UseSecondToken),
            },
        },

        Rule {
            name: "Jodoushi to adjective binding",
            current: TokenMatcher {
                pos1: Some(UnidicTag::Jodoushi),
                conjugation_type_fn: Some(|tag| {
                    matches!(
                        *tag,
                        UnidicTag::JodoushiTa | 
                        UnidicTag::JodoushiNai |
                        UnidicTag::JodoushiTai
                    )
                }),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Some(UnidicTag::Keiyoushi),
                ..Default::default()
            }),
            prev_word_pos: None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        //TODO: 
        //1. Adjectival noun + ni = adverb
        //2. Conjunctive te and de to verbs
        //3. Joudoshi to verb binding (tabenai, tabetakute etc)
        //4. ...
    ]
}