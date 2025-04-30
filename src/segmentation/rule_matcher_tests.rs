#[cfg(test)]
mod tests {
    use std::fs::File;

    use vibrato::Tokenizer;

    use crate::{
        core::YomineError, dictionary::DictType, segmentation::{
            rule_matcher::{parse_into_words, process_tokens}, token_models::{RawToken, UnidicToken, VibratoToken}, tokenizer::init_vibrato, word::{Word, POS}, word_rules::create_default_rules
        }
    };

    /// Extract UnidicTokens from a sentence going through the vibrato -> VibratoToken -> RawToken -> UnidicToken conversion chain
    pub fn tokenize_text(text: &str, tokenizer: &Result<Tokenizer, YomineError>) -> Vec<UnidicToken> {
        
        // Create vibrato tokenizer
        let tokenizer = match tokenizer {
            Ok(t) => t,
            Err(_) => {
                // Return empty vector for tests if dictionary not available
                eprintln!("Couldn't load dictionary for tests. Set UNIDIC_PATH or install dictionary.");
                return Vec::new();
            }
        };
        
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        
        // Go through the full conversion chain:
        // vibrato token -> VibratoToken -> UnidicToken
        worker.token_iter()
            .map(|token| {
                // Step 1: Convert to VibratoToken
                let vibrato_token = VibratoToken {
                    surface: token.surface().to_string(),
                    features: token.feature().to_string(),
                };
                
                // Step 2: Convert to RawToken
                let surface = vibrato_token.surface.clone();
                let raw_token = vibrato_token.into();
                //println!("{:?}", raw_token);
                
                // Step 3: Convert to UnidicToken
                (surface, raw_token).into()
            })
            .collect()
    }
    
    // Helper to create a RawToken from a VibratoToken
    impl From<&VibratoToken> for RawToken {
        fn from(vt: &VibratoToken) -> Self {
            let fields: Vec<&str> = vt.features.split(',').collect();
            
            // Helper to get field with default value if missing
            let get_field = |idx: usize| fields.get(idx).unwrap_or(&"*").to_string();
            
            RawToken {
                pos1: get_field(0),
                pos2: get_field(1),
                pos3: get_field(2),
                pos4: get_field(3),
                c_type: get_field(4),
                c_form: get_field(5),
                l_form: get_field(6),
                lemma: get_field(7),
                orth: get_field(8),
                pron: get_field(9),
                orth_base: get_field(10),
                pron_base: get_field(11),
                goshu: get_field(12),
                i_type: get_field(13),
                i_form: get_field(14),
                f_type: get_field(15),
                f_form: get_field(16),
                i_con_type: get_field(17),
                f_con_type: get_field(18),
                _type: get_field(19),
                kana: get_field(20),
                kana_base: get_field(21),
                form: get_field(22),
                form_base: get_field(23),
                a_type: get_field(24),
                a_con_type: get_field(25),
                a_mod_type: get_field(26),
                lid: get_field(27),
                lemma_id: get_field(28),
            }
        }
    }

    pub fn test_sentence(input: &str, expected_words: Vec<(&str, POS, usize, Option<&str>)>) -> bool {

        let tokenizer = init_vibrato(&DictType::Unidic);
        let tokens = tokenize_text(input, &tokenizer);
        
        if tokens.is_empty() {
            eprintln!("Skipping test, no tokens generated");
            return true;
        }
        
        let result = parse_into_words(tokens).expect("Failed to process tokens");
        
        if result.len() != expected_words.len() {
            eprintln!(
                "Expected {} words but got {} for sentence: '{}'", 
                expected_words.len(), 
                result.len(), 
                input
            );
            
            eprintln!("Got words:");
            for (i, word) in result.iter().enumerate() {
                eprintln!(
                    "  {}. '{}' ({:?}), main_word: {:?}", 
                    i+1, word.surface_form, word.part_of_speech, word.main_word
                );
            }
            
            eprintln!("Expected words:");
            for (i, (surface, pos, _, main_word)) in expected_words.iter().enumerate() {
                eprintln!("  {}. '{}' ({:?}), main_word: {:?}", i+1, surface, pos, main_word);
            }
            
            return false;
        }
        
        for (i, (word, (expected_surface, expected_pos, expected_token_count, expected_main_word))) in 
            result.iter().zip(expected_words.iter()).enumerate() {
            
            let main_word_matches = match (&word.main_word, expected_main_word) {
                (Some(main), Some(expected)) => main.lemma_form == *expected,
                (None, None) => true,
                _ => false,
            };
            
            if word.surface_form != *expected_surface 
               || word.part_of_speech != *expected_pos 
               || word.tokens.len() != *expected_token_count
               || !main_word_matches {
                
                eprintln!(
                    "Word {} mismatch: got '{}' ({:?}, {} tokens, main_word: {:?}) but expected '{}' ({:?}, {} tokens, main_word: {:?})",
                    i+1,
                    word.surface_form, word.part_of_speech, word.tokens.len(), word.main_word,
                    expected_surface, expected_pos, expected_token_count, expected_main_word
                );
                
                return false;
            } else {
                println!(
                    "Word {} match: '{}' ({:?}, {} tokens, main_word: {:?})",
                    i+1, word.surface_form, word.part_of_speech, word.tokens.len(), word.main_word
                );
                
                if word.tokens.len() > 1 {
                    println!("  Combined tokens:");
                    for (j, token) in word.tokens.iter().enumerate() {
                        println!("    {}. '{}'", j+1, token.surface);
                    }
                }
            }
        }
        
        true
    }
    
    
    #[test]
    fn inspect_tokens() {
        let test_phrases = vec![
             // Basic noun and particle
            // "犬がいます。",  // There is a dog.

            // // Verb with polite form
            // "食べます。",  // I eat.

            // // Verb with negative form
            // "食べません。",  // I do not eat.

            // // Adjective with copula
            //"高いです。",  // It is expensive.

            // // Adjective with past tense
            //"高かったです。",  // It was expensive.

            // // Noun with suffix
            //"先生です。",  // It is a teacher.

            // // Verb with multiple auxiliaries
            //"食べられません。",  // I cannot eat.

            // // Adjective with negative form
            // "高くありません。",  // It is not expensive. (polite)
            // "高くない",  // It is not expensive.

            // // Adverb modifying a verb
            //"早く走ります。",  // I run quickly.

            // // Compound noun
            //"日本語教師",  // Japanese language teacher

            // // Verb with prefix
            //"再確認します。",  // I will reconfirm.

            // // Compound proper noun + past tense verb
            //"東京タワーに行きました。",  // I went to Tokyo Tower.

            // // Verb with causative-passive auxiliaries
            //"食べさせられました。",  // I was made to eat.

            // // Number with counter (books)
            //"三冊の本を買いました。",  // I bought three books.

            // Numbers (like 3 thousand 5 hundred) 
            // "三千五百円",  // 3500 yen
            // "50百円",  

            //Extract suru verbs and na adjectives for proper sentences highlighting
            // "勉強することが好きです。",  // should extract 勉強, but highlight 勉強する as one unit. 
            //"彼は元気な人です。",  // should extract 元気, but highlight 元気な as one unit.
            "ごちそうさまでした",
            // // Large text.
            //"東京に住んでいる日本語教師の田中さんは、毎朝早く起きて、朝ごはんを食べますが、今日は特別に早く起きました。電車に乗って、学校へ行く途中、友達に会って、一緒に学校まで行きました。授業で、学生に日本語を教える時、田中さんはいつも熱心に説明します。田中さんが教えている学生は、とても優秀です。お昼に、同僚とラーメンを食べに行きましたが、あまり美味しくなかったです。午後、東京タワーに登りましたが、田中さんは高いところが苦手なので、すぐに降りました。夕方、家に帰って、疲れていたので、早く寝ました。田中さんは三冊の本を買いましたが、猫を好きです。田中さんは、教えることが好きです。東京タワーは高いですか？"
        ];
    
        let tokenizer = init_vibrato(&DictType::Unidic);
    
        for sentence in test_phrases {
            println!("\n===== Analyzing sentence: \"{}\" =====", sentence);
            let tokens = tokenize_text(sentence, &tokenizer);
            
            if tokens.is_empty() {
                println!("  Tokenizer not available, skipping");
                continue;
            }
            
            let words = parse_into_words(tokens.clone()).expect("Failed to process tokens");
            println!("Processed words ({}):", words.len());
            for (i, word) in words.iter().enumerate() {
                match &word.main_word {
                    Some(mw) => println!("  {}. \"{}[{}]\" (POS: {:?}, token count: {}, main_word: {:?})", 
                    i+1, word.surface_form, word.surface_hatsuon, word.part_of_speech, word.tokens.len(), mw.lemma_form),
                    None => println!("  {}. \"{}[{}]\" (POS: {:?}, token count: {}, main_word: None)", 
                            i+1, word.surface_form, word.surface_hatsuon, word.part_of_speech, word.tokens.len()),
                }
                
                
                for (j, token) in word.tokens.iter().enumerate() {
                    println!("       \"{}[{}]\" (POS: {:?}, {:?}, {:?}, {:?}, conjugation_type: {:?}, conjugation_form: {:?})", 
                            token.surface, token.surface_hatsuon, token.pos1, token.pos2, token.pos3, token.pos4,
                            token.conjugation_type, token.conjugation_form);
                }
                
            }
            
            println!("\nTest assertion format:");
            println!("vec![");
            for word in words.iter() {
                let main_word_str = match &word.main_word {
                    Some(mw) => format!("Some(\"{}\")", mw.lemma_form),
                    None => "None".to_string()
                };
                println!("    (\"{}\", POS::{:?}, {}, {}),", 
                        word.surface_form, word.part_of_speech, word.tokens.len(), main_word_str);
            }
            println!("]");
        }
        
        assert!(true);
    }
}