use std::{
    fs,
    sync::LazyLock,
};

use regex::Regex;
use rsubs_lib::{
    SRT,
    SSA,
};

use crate::core::{
    models::{
        SourceFileType,
        TimeStamp,
    },
    Sentence,
    SourceFile,
    YomineError,
};

// Regex now handles any parathesis (full or half width) that contains only hiragana. Not sure if we should include Katakana
// but that would easy to add, just add '\p{scx=Katakana}'
static KANA_READING_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\(|（)[\p{scx=Hiragana}・･\s]+(?:\)|）)")
        .expect("Failed to compile kana-reading regex")
});

static STRIP_INLINE_TAGS: LazyLock<Regex> = LazyLock::new(|| {
    // Any HTML/WebVTT-style tag (a whitelist kept leaking) + ASS {\...} overrides.
    Regex::new(r"(?i)</?[a-z][^<>]*>|\{\\[^}]*\}")
        .expect("Failed to compile inline_strip_tags regex")
});

// Single pass so &amp;lt; yields the literal &lt; instead of double-decoding.
fn decode_html_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let decoded = rest[1..].find(';').filter(|&i| i <= 8).and_then(|i| {
            let name = &rest[1..1 + i];
            let c = match name {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                _ if name.starts_with("#x") || name.starts_with("#X") => {
                    u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32)
                }
                _ if name.starts_with('#') => {
                    name[1..].parse::<u32>().ok().and_then(char::from_u32)
                }
                _ => None,
            };
            c.map(|c| (c, 1 + i + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Shared subtitle-text cleanup: decode HTML entities, collapse whitespace,
/// strip kana-reading parentheses and inline styling tags. Used by the
/// subtitle parsers and the asbplayer subtitle importer (issue #105).
pub fn clean_subtitle_text(raw: &str) -> String {
    // Entity-encoded tags (&lt;i&gt; from asbplayer/YouTube) must decode before the tag strip sees them.
    let text = decode_html_entities(raw);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = KANA_READING_REGEX.replace_all(&text, "");
    STRIP_INLINE_TAGS.replace_all(&text, "").trim().to_string()
}

fn parse_srt(srt: SRT, source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let sentences: Vec<Sentence> = srt
        .lines
        .iter()
        .filter(|s| !s.text.is_empty())
        .enumerate()
        .filter_map(|(id, entry)| {
            let text = clean_subtitle_text(&entry.text);

            if text.is_empty() {
                return None;
            }

            let timestamp = TimeStamp { start: entry.start, end: entry.end };

            Some(Ok(Sentence {
                id: id,
                source_id: source_file.id, // Reference to the SourceFile ID
                segments: vec![],          // segments are generated after tokenization
                text: text,
                timestamp: Some(timestamp),
                comprehension: 0.0, // Will be calculated after term matching
            }))
        })
        .collect::<Result<Vec<_>, YomineError>>()?;

    if sentences.is_empty() {
        return Err(YomineError::Custom("No subtitles found in the file.".to_string()));
    }

    Ok(sentences)
}

pub fn read_srt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    //So far we only know netflix uses this formatting as per (https://partnerhelp.netflixstudios.com/hc/en-us/articles/215767517-Japanese-Timed-Text-Style-Guide)
    // let delete_readings = source_file.creator.as_deref() == Some("Netflix");

    let raw_srt = fs::read_to_string(&source_file.original_file)?;
    //new rsubs doesn't like utf-8 BOM at the beginning of the file:
    let raw_srt = raw_srt.trim_start_matches('\u{feff}');
    let srt = SRT::parse(raw_srt)
        .map_err(|err| YomineError::Custom(format!("Error Parsing SRT File: {}", err)))?;

    parse_srt(srt, source_file)
}

fn read_ssa(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let raw_file = fs::read_to_string(&source_file.original_file)?;
    let raw_file = raw_file.trim_start_matches('\u{feff}');

    let ssa = SSA::parse_lenient(raw_file)
        .map_err(|err| YomineError::Custom(format!("Error Parsing SSA/ASS File: {}", err)))?;

    let srt = ssa.to_srt();

    parse_srt(srt, source_file)
}

pub fn read_txt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let raw_text = fs::read_to_string(&source_file.original_file)?;
    let raw_text = raw_text.trim_start_matches('\u{feff}');

    static SENTENCE_SPLIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"([。！？｡!?.]+)").expect("Failed to compile sentence split regex")
    });

    let mut sentences = Vec::new();
    let mut sentence_id = 0;

    for line in raw_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = SENTENCE_SPLIT_REGEX.split(line).collect();

        for part in parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Remove kana-reading parentheses and styling tags.
            let part = decode_html_entities(part);
            let text = KANA_READING_REGEX.replace_all(&part, "");
            let text = STRIP_INLINE_TAGS.replace_all(&text, "");
            let text = text.trim().to_string();

            if !text.is_empty() {
                sentences.push(Sentence {
                    id: sentence_id,
                    source_id: source_file.id,
                    segments: vec![],
                    text,
                    timestamp: None,
                    comprehension: 0.0,
                });
                sentence_id += 1;
            }
        }
    }

    if sentences.is_empty() {
        return Err(YomineError::Custom("No text found in the file.".to_string()));
    }

    Ok(sentences)
}

pub fn read(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    match source_file.file_type {
        SourceFileType::SRT => read_srt(source_file),
        SourceFileType::SSA => read_ssa(source_file),
        SourceFileType::TXT => read_txt(source_file),
        SourceFileType::Other(ref format) => Err(YomineError::UnsupportedFileType(format.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::clean_subtitle_text;

    #[test]
    fn strips_basic_styling_tags() {
        assert_eq!(clean_subtitle_text("自分の強さを<b>誇りなさい</b>"), "自分の強さを誇りなさい");
        assert_eq!(clean_subtitle_text("<b>全力</b>で走れ。"), "全力で走れ。");
        assert_eq!(clean_subtitle_text("<i>心の声</i>"), "心の声");
        assert_eq!(clean_subtitle_text(r##"<font color="#fff">台詞</font>"##), "台詞");
    }

    #[test]
    fn strips_arbitrary_tags_not_just_the_old_whitelist() {
        assert_eq!(clean_subtitle_text("<c.japanese>字幕</c>"), "字幕");
        assert_eq!(clean_subtitle_text("<span style=\"x\">言葉</span>"), "言葉");
        assert_eq!(clean_subtitle_text("<em>強調</em>と<strong>太字</strong>"), "強調と太字");
    }

    #[test]
    fn strips_ass_overrides_and_keeps_plain_text() {
        assert_eq!(clean_subtitle_text(r"{\i1}斜体{\i0}のまま"), "斜体のまま");
        assert_eq!(clean_subtitle_text("タグなしの文。"), "タグなしの文。");
    }

    #[test]
    fn decodes_entities_then_strips_the_revealed_tags() {
        assert_eq!(clean_subtitle_text("&lt;i&gt;心の声&lt;/i&gt;"), "心の声");
        assert_eq!(clean_subtitle_text("&lt;紅茶はかろうじて…&gt;"), "<紅茶はかろうじて…>");
    }

    #[test]
    fn decodes_named_and_numeric_entities() {
        assert_eq!(clean_subtitle_text("A&amp;B"), "A&B");
        assert_eq!(clean_subtitle_text("&quot;引用&quot;と&#39;引用&#39;"), "\"引用\"と'引用'");
        assert_eq!(clean_subtitle_text("&#12354;と&#x3042;"), "あとあ");
        assert_eq!(clean_subtitle_text("残り&nbsp;3日"), "残り 3日");
    }

    #[test]
    fn does_not_double_decode_or_touch_bare_ampersands() {
        assert_eq!(clean_subtitle_text("&amp;lt;"), "&lt;");
        assert_eq!(clean_subtitle_text("パンケーキ&エッグ"), "パンケーキ&エッグ");
        assert_eq!(clean_subtitle_text("値段は3&lt;5"), "値段は3<5");
    }
}
