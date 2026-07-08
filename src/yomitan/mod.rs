//! yomitan-api client (github.com/yomidevs/yomitan-api, issue #105): the
//! user's own Yomitan config supplies deck/model/templates for mined cards.

use std::collections::HashMap;

use serde::Deserialize;

use crate::core::errors::YomineError;

/// Markers Yomitan can't render without the source sentence.
const SENTENCE_MARKER: &str = "sentence";
const CLOZE_PREFIX: &str = "cloze-prefix";
const CLOZE_BODY: &str = "cloze-body";
const CLOZE_SUFFIX: &str = "cloze-suffix";

#[derive(Debug, Clone, Deserialize)]
pub struct CardFormatField {
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardFormat {
    pub name: String,
    pub deck: String,
    pub model: String,
    pub fields: HashMap<String, CardFormatField>,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    /// Base64 payload, stored into Anki via `storeMediaFile`.
    pub content: String,
    /// Filename the rendered fields already reference (e.g. `[sound:...]`).
    pub anki_filename: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedFields {
    /// One map of marker → rendered value per dictionary entry (we request one).
    #[serde(default)]
    pub fields: Vec<HashMap<String, String>>,
    #[serde(default)]
    pub audio_media: Vec<MediaItem>,
    #[serde(default)]
    pub dictionary_media: Vec<MediaItem>,
}

async fn post<T: for<'de> Deserialize<'de>>(
    base_url: &str,
    path: &str,
    body: serde_json::Value,
) -> Result<T, YomineError> {
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path);
    // 10s cap so status probes can't hang on unroutable URLs.
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build()?;
    let response = client.post(&url).json(&body).send().await?;
    Ok(response.json().await?)
}

/// Reachability probe for the settings UI / preflight.
pub async fn get_version(base_url: &str) -> Result<String, YomineError> {
    #[derive(Deserialize)]
    struct Version {
        version: String,
    }
    let v: Version = post(base_url, "yomitanVersion", serde_json::json!({})).await?;
    Ok(v.version)
}

/// The user's Yomitan Anki card formats; mining uses the first `term` format.
pub async fn get_term_card_format(base_url: &str) -> Result<CardFormat, YomineError> {
    let formats: Vec<CardFormat> = post(base_url, "ankiCardFormats", serde_json::json!({})).await?;
    formats.into_iter().find(|f| f.kind == "term").ok_or_else(|| {
        YomineError::Custom(
            "Yomitan has no term card format configured — set one up under \
             Yomitan Settings → Anki"
                .to_string(),
        )
    })
}

/// Render `markers` for `text`; `include_media` adds base64 audio/images.
pub async fn render_fields(
    base_url: &str,
    text: &str,
    markers: &[String],
    include_media: bool,
) -> Result<RenderedFields, YomineError> {
    post(
        base_url,
        "ankiFields",
        serde_json::json!({
            "text": text,
            "type": "term",
            "markers": markers,
            "maxEntries": 1,
            "includeMedia": include_media,
        }),
    )
    .await
}

/// `{marker}` tokens referenced by a card format's field templates.
pub fn collect_markers(format: &CardFormat) -> Vec<String> {
    let mut markers: Vec<String> = Vec::new();
    for field in format.fields.values() {
        for marker in parse_markers(&field.value) {
            if !markers.contains(&marker) {
                markers.push(marker);
            }
        }
    }
    markers
}

fn parse_markers(template: &str) -> Vec<String> {
    let mut markers = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('}') {
            markers.push(rest[..end].to_string());
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    markers
}

/// Source-sentence context for the markers Yomitan can't render.
pub struct SentenceContext<'a> {
    pub sentence: &'a str,
    pub term: &'a str,
}

/// Substitute rendered markers into the field templates. Empty results are
/// dropped so they never overwrite what Anki/asbplayer would fill.
pub fn assemble_fields(
    format: &CardFormat,
    rendered: &HashMap<String, String>,
    sentence_ctx: Option<SentenceContext<'_>>,
) -> HashMap<String, String> {
    let mut effective: HashMap<String, String> = rendered.clone();

    if let Some(ctx) = sentence_ctx {
        effective.insert(SENTENCE_MARKER.to_string(), ctx.sentence.to_string());
        // Cloze markers = the sentence split around the mined term.
        if let Some(pos) = ctx.sentence.find(ctx.term) {
            effective.insert(CLOZE_PREFIX.to_string(), ctx.sentence[..pos].to_string());
            effective.insert(CLOZE_BODY.to_string(), ctx.term.to_string());
            effective
                .insert(CLOZE_SUFFIX.to_string(), ctx.sentence[pos + ctx.term.len()..].to_string());
        }
    }

    format
        .fields
        .iter()
        .filter_map(|(name, field)| {
            let mut value = String::new();
            let mut rest = field.value.as_str();
            while let Some(start) = rest.find('{') {
                value.push_str(&rest[..start]);
                rest = &rest[start + 1..];
                match rest.find('}') {
                    Some(end) => {
                        if let Some(v) = effective.get(&rest[..end]) {
                            value.push_str(v);
                        }
                        rest = &rest[end + 1..];
                    }
                    None => {
                        value.push('{');
                        break;
                    }
                }
            }
            value.push_str(rest);
            if value.trim().is_empty() {
                None
            } else {
                Some((name.clone(), value))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(fields: &[(&str, &str)]) -> CardFormat {
        CardFormat {
            name: "test".into(),
            deck: "Mining".into(),
            model: "Japanese".into(),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), CardFormatField { value: v.to_string() }))
                .collect(),
            kind: "term".into(),
        }
    }

    #[test]
    fn collects_unique_markers() {
        let f = format(&[
            ("Word", "{expression}"),
            ("Meaning", "{glossary}"),
            ("Extra", "{expression}{furigana}"),
        ]);
        let mut markers = collect_markers(&f);
        markers.sort();
        assert_eq!(markers, ["expression", "furigana", "glossary"]);
    }

    #[test]
    fn substitutes_markers_and_literal_text() {
        let f = format(&[("Word", "{expression}"), ("Front", "Word: {expression} ({reading})")]);
        let rendered = HashMap::from([
            ("expression".into(), "食べる".into()),
            ("reading".into(), "たべる".into()),
        ]);
        let fields = assemble_fields(&f, &rendered, None);
        assert_eq!(fields["Word"], "食べる");
        assert_eq!(fields["Front"], "Word: 食べる (たべる)");
    }

    #[test]
    fn drops_empty_fields() {
        let f = format(&[("Word", "{expression}"), ("Sentence", "{sentence}")]);
        let rendered = HashMap::from([("expression".into(), "食べる".into())]);
        let fields = assemble_fields(&f, &rendered, None);
        assert_eq!(fields.get("Sentence"), None);
        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn sentence_context_fills_sentence_and_cloze() {
        let f = format(&[
            ("Sentence", "{sentence}"),
            ("Cloze", "{cloze-prefix}[{cloze-body}]{cloze-suffix}"),
        ]);
        let rendered = HashMap::new();
        let ctx = SentenceContext { sentence: "毎日パンを食べる。", term: "食べる" };
        let fields = assemble_fields(&f, &rendered, Some(ctx));
        assert_eq!(fields["Sentence"], "毎日パンを食べる。");
        assert_eq!(fields["Cloze"], "毎日パンを[食べる]。");
    }

    #[test]
    fn unclosed_brace_is_literal() {
        let f = format(&[("Odd", "{expression} {oops")]);
        let rendered = HashMap::from([("expression".into(), "犬".into())]);
        let fields = assemble_fields(&f, &rendered, None);
        assert_eq!(fields["Odd"], "犬 {oops");
    }
}
