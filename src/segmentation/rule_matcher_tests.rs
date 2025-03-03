#[cfg(test)]
mod tests {
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
                    feature: token.feature().to_string(),
                };
                
                // Step 2: Convert to RawToken
                let raw_token = create_raw_token_from_vibrato(&vibrato_token);
                //println!("{:?}", raw_token);
                
                // Step 3: Convert to UnidicToken
                (vibrato_token.surface, raw_token).into()
            })
            .collect()
    }
    
    // Helper to create a RawToken from a VibratoToken
    fn create_raw_token_from_vibrato(vt: &VibratoToken) -> RawToken {
        let fields: Vec<&str> = vt.feature.split(',').collect();
        
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
                (Some(main), Some(expected)) => main == expected,
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
            "1万円だ。", //Join numbers
            "綺麗な花だ。", //Na-adj possible + na
            "勉強する。", //suru possible noun + suru
            "勉強します。", //last case + masu
            "生徒たちが笑う", //suffix noun
            "お昼ご飯です", //prefix noun
            "お飲みになります", //prefix verb
            "昨日は早かったです", //past adjective
            "今日は旅館も紹介してみようと思いました。", //test
        ];

        let tokenizer = init_vibrato(&DictType::Unidic);
        
        for sentence in test_phrases {
            println!("\n===== Analyzing sentence: \"{}\" =====", sentence);
            let tokens = tokenize_text(sentence, &tokenizer);
            
            if tokens.is_empty() {
                println!("  Tokenizer not available, skipping");
                continue;
            }
            
            println!("Raw tokens ({}):", tokens.len());
            for (i, token) in tokens.iter().enumerate() {
                println!("  {}. \"{}\" (POS: {:?}, {:?}, {:?}, {:?}, conjugation_type: {:?}, conjugation_form: {:?})", 
                         i+1, token.surface, token.pos1, token.pos2, token.pos3, token.pos4, 
                         token.conjugation_type, token.conjugation_form);
            }
            
            let words = parse_into_words(tokens.clone()).expect("Failed to process tokens");
            println!("Processed words ({}):", words.len());
            for (i, word) in words.iter().enumerate() {
                println!("  {}. \"{}\" (POS: {:?}, token count: {}, main_word: {:?})", 
                         i+1, word.surface_form, word.part_of_speech, word.tokens.len(), word.main_word);
                
                if word.tokens.len() > 1 {
                    println!("     Tokens in word:");
                    for (j, token) in word.tokens.iter().enumerate() {
                        println!("       {}. \"{}\" (POS: {:?}, {:?}, {:?}, {:?}, conjugation_type: {:?}, conjugation_form: {:?})", 
                                j+1, token.surface, token.pos1, token.pos2, token.pos3, token.pos4,
                                token.conjugation_type, token.conjugation_form);
                    }
                }
            }
            
            println!("\nTest assertion format:");
            println!("vec![");
            for word in words.iter() {
                let main_word_str = match &word.main_word {
                    Some(mw) => format!("Some(\"{}\")", mw),
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