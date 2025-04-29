use super::rule_matcher::{Rule, TokenMatcher, RuleAction, MainWordPolicy, Matcher};
use super::word::POS;
use super::unidic_tags::UnidicTag;

/**
 * These rules are not necessarily how an online parser would parse like ichi.moe. We want to prioritize minable terms and what we will be adding and checking against in our dictionary
 * */
pub fn create_default_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "Jodoushi to jodoushi binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                conjugation_type: Matcher::Any(vec![
                    UnidicTag::JodoushiTa,
                    UnidicTag::JodoushiNai,
                    UnidicTag::JodoushiTai,
                    UnidicTag::JodoushiMasu,
                    UnidicTag::JodoushiNu
                ]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "Verb + Tai binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                conjugation_type: Matcher::Any(vec![UnidicTag::JodoushiTai]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Doushi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: Some(POS::Adjective),
                main_word_policy: None,
            },
        },

        Rule {
            name: "Jodoushi to verb binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                conjugation_type: Matcher::Not(vec![UnidicTag::JodoushiTai]),
                surface: Matcher::Not(vec!["な".to_string()]),
                ..Default::default()
            },
            next: None,
            prev: None,
            prev_word_pos: Matcher::Any(vec![POS::Verb, POS::SuruVerb]),
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "Jodoushi to adjective binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                conjugation_type: Matcher::Any(vec![
                    UnidicTag::JodoushiTa,
                    UnidicTag::JodoushiNai,
                    UnidicTag::JodoushiTai
                ]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Keiyoushi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "Keiyoushi to Hijiritsukanou Keiyoushi binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Keiyoushi]),
                pos2: Matcher::Any(vec![UnidicTag::Hijiritsukanou]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Keiyoushi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "Compound Noun Binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                pos2: Matcher::Any(vec![UnidicTag::Futsuumeishi]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                pos2: Matcher::Any(vec![UnidicTag::Koyuumeishi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::Not(vec![POS::CompoundNoun]),
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: Some(POS::CompoundNoun),
                main_word_policy: None,
            },
        },

        Rule {
            name: "Prefix Noun",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Settouji]),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: Matcher::None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::Noun,
                main_word_policy: Some(MainWordPolicy::UseSecondToken),
            },
        },

        Rule {
            name: "Te-form binding",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Joshi]),
                pos2: Matcher::Any(vec![UnidicTag::Setsuzokujoshi]),
                surface: Matcher::Any(vec!["て".to_string(), "で".to_string()]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Doushi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "Suffix to noun",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Setsubiji]),
                ..Default::default()
            },
            next: None,
            prev: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                ..Default::default()
            }),
            prev_word_pos: Matcher::None,
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None, 
            },
        },

        Rule {
            name: "Join numbers",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                pos2: Matcher::Any(vec![UnidicTag::Suushi]),
                ..Default::default()
            },
            next: None,
            prev: None,
            prev_word_pos: Matcher::Any(vec![POS::Number]),
            action: RuleAction::MergeWithPrevious {
                attach_prev: true,
                attach_prev_lemma: true,
                update_prev_pos: None,
                main_word_policy: None,
            },
        },

        Rule {
            name: "suru-possible + suru",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                pos3: Matcher::Any(vec![UnidicTag::Sahenkanou]),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                conjugation_type: Matcher::Any(vec![UnidicTag::Sagyouhenkaku]),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: Matcher::None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::SuruVerb,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },

        //We need two rules since unidic has both na and na-possible adjectival nouns
        Rule {
            name: "Na-adj + na",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Keijoushi]),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                surface: Matcher::Any(vec!["な".to_string()]),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: Matcher::None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::AdjectivalNoun,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },

        Rule {
            name: "Na-adj possible + na",
            current: TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Meishi]),
                pos3: Matcher::Any(vec![UnidicTag::Keijoushikanou]),
                ..Default::default()
            },
            next: Some(TokenMatcher {
                pos1: Matcher::Any(vec![UnidicTag::Jodoushi]),
                surface: Matcher::Any(vec!["な".to_string()]),
                ..Default::default()
            }),
            prev: None,
            prev_word_pos: Matcher::None,
            action: RuleAction::CreateWord {
                eat_next: true,
                eat_next_lemma: true,
                pos: POS::AdjectivalNoun,
                main_word_policy: Some(MainWordPolicy::UseFirstToken),
            },
        },

        //
        // Rule {
        //     name: "Hijiritsukanou keiyoushi",
        //     current: TokenMatcher {
        //         pos1: Matcher::Any(vec![UnidicTag::Keiyoushi]),
        //         pos2: Matcher::Any(vec![UnidicTag::Hijiritsukanou]),
        //         ..Default::default()
        //     },
        //     next: None,
        //     prev: None,
        //     prev_word_pos: Matcher::None,
        //     action: RuleAction::MergeWithPrevious {
        //         attach_prev: true,
        //         attach_prev_lemma: true,
        //         update_prev_pos: None,
        //         main_word_policy: None,
        //     },
        // },



        // Rule {
        //     name: "Prefix Verb",
        //     current: TokenMatcher {
        //         pos1: Matcher::Any(vec![UnidicTag::Settouji]),
        //         ..Default::default()
        //     },
        //     next: Some(TokenMatcher {
        //         pos1: Matcher::Any(vec![UnidicTag::Doushi]),
        //         ..Default::default()
        //     }),
        //     prev: None,
        //     prev_word_pos: Matcher::None,
        //     action: RuleAction::CreateWord {
        //         eat_next: true,
        //         eat_next_lemma: true,
        //         pos: POS::Verb,
        //         main_word_policy: Some(MainWordPolicy::UseSecondToken),
        //     },
        // },

    ]
}