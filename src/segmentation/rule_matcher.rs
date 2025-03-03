use crate::core::YomineError;
use super::word::{Word, POS, get_default_pos};
use super::token_models::UnidicToken;
use super::unidic_tags::UnidicTag;
use super::word_rules::create_default_rules;

/**
 * Allows us to write rules instead of lots of nested logic which can become unwieldy. 
 * 
 */

#[derive(Default)]
pub struct TokenMatcher {
    pub pos1: Option<UnidicTag>,
    pub pos2: Option<UnidicTag>,
    pub pos3: Option<UnidicTag>,
    pub pos4: Option<UnidicTag>,
    pub surface: Option<String>,
    pub conjugation_type: Option<UnidicTag>,
    pub conjugation_form: Option<UnidicTag>,
    pub pos2_fn: Option<fn(&UnidicTag) -> bool>,
    pub conjugation_type_fn: Option<fn(&UnidicTag) -> bool>,
}

impl TokenMatcher {
    pub fn matches(&self, token: &UnidicToken) -> bool {
        self.pos1.as_ref().map_or(true, |tag| *tag == token.pos1) &&
        (self.pos2.as_ref().map_or(true, |tag| *tag == token.pos2) && 
         self.pos2_fn.map_or(true, |f| f(&token.pos2))) &&
        self.pos3.as_ref().map_or(true, |tag| *tag == token.pos3) &&
        self.pos4.as_ref().map_or(true, |tag| *tag == token.pos4) &&
        self.surface.as_ref().map_or(true, |s| s == &token.surface) &&
        (self.conjugation_type.as_ref().map_or(true, |tag| *tag == token.conjugation_type) &&
         self.conjugation_type_fn.map_or(true, |f| f(&token.conjugation_type))) &&
        self.conjugation_form.as_ref().map_or(true, |tag| *tag == token.conjugation_form)
    }
}

#[derive(Clone)]
pub enum RuleAction {
    CreateWord {
        eat_next: bool,
        eat_next_lemma: bool,
        pos: POS,
        main_word_policy: Option<MainWordPolicy>,
    },

    MergeWithPrevious {
        attach_prev: bool,
        attach_prev_lemma: bool,
        update_prev_pos: Option<POS>,
        main_word_policy: Option<MainWordPolicy>,
    },
}

#[derive(Clone)]
pub enum MainWordPolicy {
    UseFirstToken,   // Use first token's lemma as main word
    UseSecondToken,  // Use second token's lemma as main word
}

pub struct Rule {
    pub name: &'static str,
    pub current: TokenMatcher,
    pub next: Option<TokenMatcher>,
    pub prev: Option<TokenMatcher>,
    pub prev_word_pos: Option<POS>,
    pub action: RuleAction,
}

pub fn process_tokens(tokens: Vec<UnidicToken>, rules: &[Rule]) -> Result<Vec<Word>, YomineError> {
    let mut words: Vec<Word> = Vec::new();
    let mut tokens_iter = tokens.into_iter().peekable();
    let mut prev_token: Option<UnidicToken> = None;

    while let Some(current_token) = tokens_iter.next() {
        let prev_word_pos = words.last().map(|w| &w.part_of_speech);
        let next_token = tokens_iter.peek();
        
        // Try to find a matching rule
        let mut rule_applied = false;
        for rule in rules {
            // Check all rule conditions
            let current_matches = rule.current.matches(&current_token);
            
            let next_matches = match (&rule.next, next_token) {
                (Some(matcher), Some(token)) => matcher.matches(token),
                (Some(_), None) => false,
                (None, _) => true,
            };
            
            let prev_token_matches = match (&rule.prev, &prev_token) {
                (Some(matcher), Some(token)) => matcher.matches(token),
                (Some(_), None) => false,
                (None, _) => true,
            };
            
            let prev_word_matches = match (&rule.prev_word_pos, prev_word_pos) {
                (Some(required_pos), Some(word_pos)) => required_pos == word_pos,
                (Some(_), None) => false,
                (None, _) => true,
            };

            // If all conditions match, apply the rule
            if current_matches && next_matches && prev_token_matches && prev_word_matches {
                match &rule.action {
                    RuleAction::CreateWord { eat_next, eat_next_lemma, pos, main_word_policy } => {
                        let mut word = Word {
                            surface_form: current_token.surface.clone(),
                            surface_hatsuon: current_token.surface_hatsuon.clone(),
                            lemma_form: current_token.lemma_form.clone(),
                            lemma_hatsuon: current_token.lemma_hatsuon.clone(),
                            part_of_speech: pos.clone(),
                            tokens: vec![current_token.clone()],
                            main_word: None,
                        };

                        // Set main word based on policy if this is a single-token word
                        if let Some(MainWordPolicy::UseFirstToken) = main_word_policy {
                            word.main_word = Some(current_token.lemma_form.clone());
                        }

                        if *eat_next && next_token.is_some() {
                            if let Some(next) = tokens_iter.next() {
                                // Handle main word policy for multi-token words
                                match main_word_policy {
                                    Some(MainWordPolicy::UseFirstToken) => {
                                        // Already set above
                                    },
                                    Some(MainWordPolicy::UseSecondToken) => {
                                        word.main_word = Some(next.lemma_form.clone());
                                    },
                                    None => {
                                        // No main word policy
                                    }
                                }
                                
                                word.surface_form.push_str(&next.surface);
                                word.surface_hatsuon.push_str(&next.surface_hatsuon);
                                
                                if *eat_next_lemma {
                                    word.lemma_form.push_str(&next.lemma_form);
                                    word.lemma_hatsuon.push_str(&next.lemma_hatsuon);
                                }
                                
                                word.tokens.push(next.clone());
                                prev_token = Some(next); 
                            }
                        } else {
                            prev_token = Some(current_token.clone()); 
                        }

                        words.push(word);
                    },
                    RuleAction::MergeWithPrevious { attach_prev, attach_prev_lemma, update_prev_pos, main_word_policy } => {
                        if let Some(prev_word) = words.last_mut() {
                            // Apply main word policy if specified
                            if let Some(policy) = main_word_policy {
                                match policy {
                                    MainWordPolicy::UseFirstToken => {
                                        // In a merge, the first token is from the previous word
                                        if prev_word.main_word.is_none() && !prev_word.tokens.is_empty() {
                                            prev_word.main_word = Some(prev_word.tokens[0].lemma_form.clone());
                                        }
                                    },
                                    MainWordPolicy::UseSecondToken => {
                                        // In a merge, the second token is the current token
                                        prev_word.main_word = Some(current_token.lemma_form.clone());
                                    },
                                }
                            }
                            
                            if *attach_prev {
                                prev_word.surface_form.push_str(&current_token.surface);
                                prev_word.surface_hatsuon.push_str(&current_token.surface_hatsuon);
                            }
                            
                            if *attach_prev_lemma {
                                prev_word.lemma_form.push_str(&current_token.lemma_form);
                                prev_word.lemma_hatsuon.push_str(&current_token.lemma_hatsuon);
                            }
                            
                            if let Some(pos) = update_prev_pos {
                                prev_word.part_of_speech = pos.clone();
                            }
                            
                            prev_word.tokens.push(current_token.clone());
                            prev_token = Some(current_token.clone());
                        } else {
                            return Err(YomineError::Custom(format!(
                                "Rule '{}' tried to merge with previous word, but no previous word exists",
                                rule.name
                            )));
                        }
                    }
                }
                
                rule_applied = true;
                break;
            }
        }

        if !rule_applied {
            let pos = get_default_pos(&current_token);
            let word = Word {
                surface_form: current_token.surface.clone(),
                surface_hatsuon: current_token.surface_hatsuon.clone(),
                lemma_form: current_token.lemma_form.clone(),
                lemma_hatsuon: current_token.lemma_hatsuon.clone(),
                part_of_speech: pos,
                tokens: vec![current_token.clone()],
                main_word: None, // For single tokens, main_word is typically not needed
            };
            
            words.push(word);
            prev_token = Some(current_token);
        }
    }

    Ok(words)
}

/// Process tokens into words using the default ruleset
pub fn parse_into_words(tokens: Vec<UnidicToken>) -> Result<Vec<Word>, YomineError> {
    let rules = create_default_rules();
    process_tokens(tokens, &rules)
}