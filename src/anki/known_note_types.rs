//! Common Japanese mining note types, whose field roles are known from their published
//! templates (tests/fixtures/note_types/fields.json).

pub struct KnownNoteType {
    pub name: &'static str,
    /// Lowercase text found in the note type's name in Anki.
    name_contains: &'static str,
    /// Besides the role fields, what identifies a renamed copy.
    signature: &'static [&'static str],
    pub term: &'static str,
    pub reading: &'static str,
    pub sentence: &'static str,
    pub sentence_audio: &'static str,
    pub picture: &'static str,
}

impl KnownNoteType {
    pub fn roles(&self) -> [&'static str; 5] {
        [self.term, self.reading, self.sentence, self.sentence_audio, self.picture]
    }
}

const fn lapis_family(
    name: &'static str,
    name_contains: &'static str,
    signature: &'static [&'static str],
) -> KnownNoteType {
    KnownNoteType {
        name,
        name_contains,
        signature,
        term: "Expression",
        reading: "ExpressionReading",
        sentence: "Sentence",
        sentence_audio: "SentenceAudio",
        picture: "Picture",
    }
}

/// For renamed copies the first match wins, so Kiku (Lapis plus fields) comes before Lapis,
/// and Lapis before Lapis Simplified (Lapis minus fields).
const KNOWN: &[KnownNoteType] = &[
    lapis_family("Kiku", "kiku", &["RelatedExpression", "MainDefinition", "MiscInfo"]),
    lapis_family("Lapis", "lapis", &["ExpressionFurigana", "MainDefinition", "MiscInfo"]),
    lapis_family("Lapis Simplified", "lapis simplified", &["MainDefinition", "MiscInfo"]),
    KnownNoteType {
        name: "Kaishi 1.5k",
        name_contains: "kaishi",
        signature: &["Word Meaning", "Pitch Accent Notes"],
        term: "Word",
        reading: "Word Reading",
        sentence: "Sentence",
        sentence_audio: "Sentence Audio",
        picture: "Picture",
    },
    KnownNoteType {
        name: "Senren",
        name_contains: "senren",
        signature: &["wordAudio", "dictionaryPreference"],
        term: "word",
        reading: "reading",
        sentence: "sentence",
        sentence_audio: "sentenceAudio",
        picture: "picture",
    },
    KnownNoteType {
        name: "Anime Cards",
        name_contains: "anime card",
        signature: &["Glossary", "Audio", "Graph"],
        term: "Word",
        reading: "Reading",
        sentence: "Sentence",
        sentence_audio: "SentenceAudio",
        picture: "Picture",
    },
];

/// By name, otherwise by fields for a copy the user renamed.
/// By name, otherwise by fields for a copy the user renamed.
pub fn detect(model_name: &str, fields: &[String]) -> Option<&'static KnownNoteType> {
    let name = model_name.to_lowercase();
    let has = |f: &&str| fields.iter().any(|field| field == f);
    // The longest match, so "Lapis Simplified" isn't taken for Lapis.
    KNOWN
        .iter()
        .filter(|k| name.contains(k.name_contains))
        .max_by_key(|k| k.name_contains.len())
        .or_else(|| KNOWN.iter().find(|k| k.roles().iter().chain(k.signature).all(has)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::anki::field_guessing::{
        guess_picture_field,
        guess_sentence_audio_field,
        guess_sentence_field,
    };

    #[derive(serde::Deserialize)]
    struct Fixture {
        name: String,
        source: String,
        fields: Vec<String>,
    }

    fn fixtures() -> Vec<Fixture> {
        serde_json::from_str(include_str!("../../tests/fixtures/note_types/fields.json")).unwrap()
    }

    #[test]
    fn every_published_note_type_is_detected_by_name_and_by_fields() {
        for f in fixtures() {
            assert_eq!(
                detect(&f.name, &f.fields).map(|k| k.name),
                Some(f.name.as_str()),
                "{}",
                f.source
            );
            let renamed = detect("Mining", &f.fields).map(|k| k.name);
            assert_eq!(renamed, Some(f.name.as_str()), "renamed {}", f.source);
            let known = detect(&f.name, &f.fields).unwrap();
            for role in known.roles() {
                assert!(f.fields.iter().any(|field| field == role), "{role} in {}", f.source);
            }
        }
    }

    #[test]
    fn general_guesses_agree_with_the_known_maps() {
        let no_templates = HashMap::new();
        for f in fixtures() {
            let known = detect(&f.name, &f.fields).unwrap();
            let guesses = (
                guess_sentence_field(&HashMap::new(), &f.fields),
                guess_sentence_audio_field(&f.fields, &no_templates),
                guess_picture_field(&f.fields, &no_templates),
            );
            let expected = (
                Some(known.sentence.to_string()),
                Some(known.sentence_audio.to_string()),
                Some(known.picture.to_string()),
            );
            assert_eq!(guesses, expected, "{}", f.source);
        }
    }
}
