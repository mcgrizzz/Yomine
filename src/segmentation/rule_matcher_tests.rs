#[cfg(test)]
mod tests {
    use vibrato::Tokenizer;

    use crate::{
        core::YomineError,
        dictionary::token_dictionary::DictType,
        segmentation::{
            rule_matcher::parse_into_words,
            token_models::{
                UnidicToken,
                VibratoToken,
            },
            tokenizer::init_vibrato,
        },
    };

    /// Extract UnidicTokens from a sentence going through the vibrato -> VibratoToken -> RawToken -> UnidicToken conversion chain
    pub fn tokenize_text(
        text: &str,
        tokenizer: &Result<Tokenizer, YomineError>,
    ) -> Vec<UnidicToken> {
        // Create vibrato tokenizer
        let tokenizer = match tokenizer {
            Ok(t) => t,
            Err(_) => {
                // Return empty vector for tests if dictionary not available
                eprintln!(
                    "Couldn't load dictionary for tests. Set UNIDIC_PATH or install dictionary."
                );
                return Vec::new();
            }
        };

        let mut worker = tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();

        // Go through the full conversion chain:
        // vibrato token -> VibratoToken -> UnidicToken
        worker
            .token_iter()
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
            "ごちそうさまでした", //"寿司ではなくチョコレートケーキが食べたい"
                                  // // Large text.
                                  //"東京に住んでいる日本語教師の田中さんは、毎朝早く起きて、朝ごはんを食べますが、今日は特別に早く起きました。電車に乗って、学校へ行く途中、友達に会って、一緒に学校まで行きました。授業で、学生に日本語を教える時、田中さんはいつも熱心に説明します。田中さんが教えている学生は、とても優秀です。お昼に、同僚とラーメンを食べに行きましたが、あまり美味しくなかったです。午後、東京タワーに登りましたが、田中さんは高いところが苦手なので、すぐに降りました。夕方、家に帰って、疲れていたので、早く寝ました。田中さんは三冊の本を買いましたが、猫を好きです。田中さんは、教えることが好きです。東京タワーは高いですか？"
        ];

        let tokenizer = init_vibrato(&DictType::Unidic, None);

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
                    Some(mw) => println!(
                        "  {}. \"{}[{}]\" (POS: {:?}, token count: {}, main_word: {:?})",
                        i + 1,
                        word.surface_form,
                        word.surface_hatsuon,
                        word.part_of_speech,
                        word.tokens.len(),
                        mw.lemma_form
                    ),
                    None => println!(
                        "  {}. \"{}[{}]\" (POS: {:?}, token count: {}, main_word: None)",
                        i + 1,
                        word.surface_form,
                        word.surface_hatsuon,
                        word.part_of_speech,
                        word.tokens.len()
                    ),
                }

                for (_j, token) in word.tokens.iter().enumerate() {
                    println!(
                        "       \"{}[{}]\" (POS: {:?}, {:?}, {:?}, {:?}, conjugation_type: {:?}, conjugation_form: {:?})",
                        token.surface,
                        token.surface_hatsuon,
                        token.pos1,
                        token.pos2,
                        token.pos3,
                        token.pos4,
                        token.conjugation_type,
                        token.conjugation_form
                    );
                }
            }

            println!("\nTest assertion format:");
            println!("vec![");
            for word in words.iter() {
                let main_word_str = match &word.main_word {
                    Some(mw) => format!("Some(\"{}\")", mw.lemma_form),
                    None => "None".to_string(),
                };
                println!(
                    "    (\"{}\", POS::{:?}, {}, {}),",
                    word.surface_form,
                    word.part_of_speech,
                    word.tokens.len(),
                    main_word_str
                );
            }
            println!("]");
        }

        assert!(true);
    }
}

/// Real UniDic feature strings, so these run without the dictionary installed.
#[cfg(test)]
mod sahen_suffix {
    use crate::{
        core::models::Term,
        segmentation::{
            rule_matcher::parse_into_words,
            token_models::UnidicToken,
            word::POS,
        },
    };

    const KEITAI: &str = "名詞,普通名詞,一般,*,*,*,ケイタイ,形態,形態,ケータイ,形態,ケータイ,漢,*,*,*,*,*,*,体,ケイタイ,ケイタイ,ケイタイ,ケイタイ,0,C2,*,3024215389381120,11002";
    const KA: &str = "接尾辞,名詞的,サ変可能,*,*,*,カ,化,化,カ,化,カ,漢,*,*,*,*,*,*,接尾体,カ,カ,カ,カ,*,C4,*,1533002710655488,5577";
    const SA: &str = "動詞,非自立可能,*,*,サ行変格,未然形-サ,スル,為る,さ,サ,する,スル,和,*,*,*,*,*,*,用,サ,スル,サ,スル,0,C5,*,5370298291593794,19537";
    const SE: &str = "助動詞,*,*,*,下一段-サ行,連用形-一般,セル,せる,せ,セ,せる,セル,和,*,*,*,*,*,*,助動,セ,セル,セ,セル,*,動詞%F3@1,M4@1,5595157009408641,20355";
    const SERU: &str = "助動詞,*,*,*,下一段-サ行,終止形-一般,セル,せる,せる,セル,せる,セル,和,*,*,*,*,*,*,助動,セル,セル,セル,セル,*,動詞%F3@1,*,5595157009408683,20355";
    const TE: &str = "助詞,接続助詞,*,*,*,*,テ,て,て,テ,て,テ,和,*,*,*,*,*,*,接助,テ,テ,テ,テ,*,動詞%F1,*,6837321680953856,24874";
    const BENKYOU: &str = "名詞,普通名詞,サ変可能,*,*,*,ベンキョウ,勉強,勉強,ベンキョー,勉強,ベンキョー,漢,*,*,*,*,*,*,体,ベンキョウ,ベンキョウ,ベンキョウ,ベンキョウ,0,C2,*,9415126692274688,34252";
    const JITSUYOU: &str = "名詞,普通名詞,サ変可能,*,*,*,ジツヨウ,実用,実用,ジツヨー,実用,ジツヨー,漢,*,*,*,*,*,*,体,ジツヨウ,ジツヨウ,ジツヨウ,ジツヨウ,0,C2,*,4944237535830528,17987";
    const SHI: &str = "動詞,非自立可能,*,*,サ行変格,連用形-一般,スル,為る,し,シ,する,スル,和,*,*,*,*,*,*,用,シ,スル,シ,スル,0,C5,*,5370298291593857,19537";
    const JIDOU: &str = "名詞,普通名詞,一般,*,*,*,ジドウ,自動,自動,ジドー,自動,ジドー,漢,*,*,*,*,*,*,体,ジドウ,ジドウ,ジドウ,ジドウ,0,C2,*,4948635582341632,18003";
    const EIGA: &str = "名詞,普通名詞,一般,*,*,*,エイガ,映画,映画,エーガ,映画,エーガ,漢,*,*,*,*,*,*,体,エイガ,エイガ,エイガ,エイガ,0,C2,*,1000839082811904,3641";
    const KAN: &str = "接尾辞,名詞的,一般,*,*,*,カン,館,館,カン,館,カン,漢,*,*,*,*,*,*,接尾体,カン,カン,カン,カン,*,C3,*,2056095367569920,7480";

    fn terms(parts: &[(&str, &str)]) -> Vec<Term> {
        let mut offset = 0;
        let tokens: Vec<UnidicToken> = parts
            .iter()
            .map(|(surface, features)| {
                let span = offset..offset + surface.len();
                offset = span.end;
                UnidicToken::from_parts(surface, features, span)
            })
            .collect();
        parse_into_words(tokens)
            .expect("rules must not error")
            .into_iter()
            .map(Term::from)
            .collect()
    }

    fn assert_ka_suru(term: &Term, full_segment: &str) {
        assert_eq!(term.surface_form, full_segment);
        assert_eq!(term.lemma_form, "化する", "citation form must survive the auxiliaries");
        assert_eq!(term.full_segment, full_segment, "deinflection needs the whole span");
    }

    #[test]
    fn a_sahen_suffix_binds_forward_to_suru() {
        let terms = terms(&[("形態", KEITAI), ("化", KA), ("さ", SA), ("せ", SE), ("て", TE)]);

        assert_eq!(
            terms.len(),
            2,
            "{:?}",
            terms.iter().map(|t| &t.full_segment).collect::<Vec<_>>()
        );
        assert_eq!(terms[0].full_segment, "形態");
        assert_eq!(terms[0].part_of_speech, POS::Noun);
        assert_ka_suru(&terms[1], "化させて");
    }

    #[test]
    fn it_binds_the_same_way_without_a_te_form() {
        let terms = terms(&[("自動", JIDOU), ("化", KA), ("さ", SA), ("せる", SERU)]);

        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0].full_segment, "自動");
        assert_ka_suru(&terms[1], "化させる");
    }

    #[test]
    fn it_binds_the_renyou_suru_stem_too() {
        let terms = terms(&[("実用", JITSUYOU), ("化", KA), ("し", SHI), ("て", TE)]);

        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0].full_segment, "実用");
        assert_ka_suru(&terms[1], "化して");
    }

    #[test]
    fn a_plain_sahen_noun_is_unchanged() {
        let terms = terms(&[("勉強", BENKYOU), ("さ", SA), ("せ", SE), ("て", TE)]);

        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].part_of_speech, POS::SuruVerb);
        assert_eq!(terms[0].lemma_form, "勉強");
        assert_eq!(terms[0].full_segment, "勉強させて");
    }

    /// 館 is a suffix but not サ変可能, so it still merges backwards into the noun.
    #[test]
    fn a_non_sahen_suffix_does_not_bind_forward() {
        let terms = terms(&[("映画", EIGA), ("館", KAN), ("し", SHI), ("て", TE)]);

        assert_eq!(terms[0].full_segment, "映画館");
        assert_eq!(terms[0].part_of_speech, POS::Noun);
        assert!(terms.iter().all(|t| t.lemma_form != "館する"), "館 must not bind forward");
    }
}
