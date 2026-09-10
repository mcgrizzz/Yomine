//! Snapshot taken before shared phrase processing was extracted.
use yomine::{
    core::Sentence,
    dictionary::{
        frequency_dict::FrequencyDictionary,
        frequency_manager::FrequencyManager,
        token_dictionary::DictType,
        JsonFrequency,
        JsonFrequencyData,
        TermMetaBankV3,
    },
    segmentation::tokenizer::{
        batch_deinflect_terms,
        extract_words,
        extract_words_for_frequency,
        init_vibrato,
    },
};
#[test]
fn production_and_corpus_keep_their_acceptance_policies() {
    let tokenizer =
        init_vibrato(&DictType::Unidic, None).expect("phrase regressions require installed UniDic");
    let entries = [
        ("形態", "けいたい", 2000),
        ("化", "か", 100),
        ("化する", "かする", 4000),
        ("形態化する", "けいたいかする", 10),
        ("要る", "いる", 700),
        ("知る", "しる", 100),
        ("つまらない", "つまらない", 1000),
        ("つまる", "つまる", 100),
        ("おばさん", "おばさん", 2000),
        ("なんとなく", "なんとなく", 2231),
        ("何となく", "なんとなく", 2016),
        ("何と無く", "なんとなく", 1686),
        ("土曜日", "どようび", 100),
        ("複合体", "ふくごうたい", 400),
    ];
    let manager = FrequencyManager::from_dictionaries(vec![FrequencyDictionary::new(
        "test".into(),
        "1".into(),
        entries
            .into_iter()
            .map(|(term, reading, rank)| TermMetaBankV3 {
                term: term.into(),
                data_type: "freq".into(),
                data: Some(JsonFrequencyData::Nested {
                    reading: reading.into(),
                    frequency: JsonFrequency::Number(rank),
                }),
            })
            .collect(),
    )]);
    let mut snapshot = Vec::new();
    for text in [
        "形態化させて",
        "要らないです",
        "知らなかった",
        "つまらなかった",
        "おばさん",
        "なんとなく",
        "土曜日の複合体",
    ] {
        let mut source = vec![Sentence {
            id: 0,
            source_id: 0,
            text: text.into(),
            segments: vec![],
            timestamp: None,
            comprehension: 0.0,
        }];
        let production = extract_words(tokenizer.new_worker(), &mut source, &manager);
        let corpus = extract_words_for_frequency(&tokenizer, &mut source, &manager, None);
        let mut deinflections: Vec<_> =
            batch_deinflect_terms(&corpus, &manager).into_iter().collect();
        deinflections.sort();
        snapshot.push(serde_json::json!({ "text": text, "production": production, "families": production.iter().map(|t| t.lexical_family.as_ref().map(|f| &f.spellings)).collect::<Vec<_>>(), "segments": source[0].segments, "corpus": corpus, "batch": deinflections }));
    }
    let actual = serde_json::to_value(snapshot).unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/phrase_processing.json");
    if std::env::var_os("YOMINE_UPDATE_PHRASE_SNAPSHOT").is_some() {
        std::fs::write(&fixture, serde_json::to_string_pretty(&actual).unwrap()).unwrap();
    }
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn promoted_phrases_cannot_be_reused_as_adjacent_source_tokens() {
    let tokenizer = init_vibrato(&DictType::Unidic, None).expect("UniDic required");
    let manager = FrequencyManager::from_dictionaries(vec![FrequencyDictionary::new(
        "test".into(),
        "1".into(),
        [("なんとなく", "なんとなく"), ("元気だなんとなく", "げんきだなんとなく")]
            .into_iter()
            .map(|(term, reading)| TermMetaBankV3 {
                term: term.into(),
                data_type: "freq".into(),
                data: Some(JsonFrequencyData::Nested {
                    reading: reading.into(),
                    frequency: if term == "なんとなく" {
                        JsonFrequency::Complex { value: 1, display_value: Some("1㋕".into()) }
                    } else {
                        JsonFrequency::Number(1)
                    },
                }),
            })
            .collect(),
    )]);
    let mut sentences = vec![Sentence {
        id: 0,
        source_id: 0,
        text: "なんとなく元気だ".into(),
        segments: vec![],
        timestamp: None,
        comprehension: 0.0,
    }];
    let production = extract_words(tokenizer.new_worker(), &mut sentences, &manager);
    let corpus = extract_words_for_frequency(&tokenizer, &mut sentences, &manager, None);
    for terms in [production, corpus] {
        assert!(terms.iter().any(|t| t.surface_form == "なんとなく"), "{terms:?}");
        assert!(terms.iter().all(|t| sentences[0].text.contains(&t.full_segment)), "{terms:?}");
    }
}
