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

/// Consecutive same-text cues this close are one line split in two, often at a scene cut.
const SPLIT_CUE_GAP: time::Duration = time::Duration::milliseconds(100);

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
    let sentences = merge_split_cues(sentences);

    if sentences.is_empty() {
        return Err(YomineError::Custom("No subtitles found in the file.".to_string()));
    }

    Ok(sentences)
}

/// Joins such cues, then numbers sentences by position, which terms refer to them by.
fn merge_split_cues(sentences: Vec<Sentence>) -> Vec<Sentence> {
    let mut merged: Vec<Sentence> = Vec::with_capacity(sentences.len());
    for sentence in sentences {
        if let Some(last) = merged.last_mut() {
            if let (Some(prev), Some(next)) = (&mut last.timestamp, &sentence.timestamp) {
                if last.text == sentence.text && next.start - prev.end <= SPLIT_CUE_GAP {
                    prev.end = prev.end.max(next.end);
                    continue;
                }
            }
        }
        merged.push(sentence);
    }
    for (id, sentence) in merged.iter_mut().enumerate() {
        sentence.id = id;
    }
    merged
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

    let ssa = SSA::parse_lenient(without_empty_blocks(raw_file))
        .map_err(|err| YomineError::Custom(format!("Error Parsing SSA/ASS File: {}", err)))?;

    let srt = ssa.to_srt();

    parse_srt(srt, source_file)
}

/// rsubs-lib splits a file into sections at blank lines and rejects an empty one, which a
/// trailing blank line (as ffmpeg writes) or two blank lines in a row produce.
fn without_empty_blocks(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut after_blank = true;
    for line in raw.lines() {
        let blank = line.trim().is_empty();
        if !(blank && after_blank) {
            out.push_str(line);
            out.push('\n');
        }
        after_blank = blank;
    }
    out.trim_end().to_string()
}

fn sentences_from_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    source_file: &SourceFile,
) -> Vec<Sentence> {
    static SENTENCE_SPLIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"([。！？｡!?.]+)").expect("Failed to compile sentence split regex")
    });

    let mut sentences = Vec::new();
    let mut sentence_id = 0;

    for line in lines {
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

    sentences
}

pub fn read_txt(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let raw_text = fs::read_to_string(&source_file.original_file)?;
    let raw_text = raw_text.trim_start_matches('\u{feff}');

    let sentences = sentences_from_lines(raw_text.lines(), source_file);

    if sentences.is_empty() {
        return Err(YomineError::Custom("No text found in the file.".to_string()));
    }

    Ok(sentences)
}

pub fn read_epub(source_file: &SourceFile) -> Result<Vec<Sentence>, YomineError> {
    let lines = crate::epub::chapter_lines(
        &source_file.original_file,
        source_file.epub_chapters.as_deref(),
    )?;

    let sentences = sentences_from_lines(lines.iter().map(String::as_str), source_file);

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
        SourceFileType::EPUB => read_epub(source_file),
        SourceFileType::Other(ref format) => Err(YomineError::UnsupportedFileType(format.clone())),
    }
}

#[cfg(test)]
mod tests {
    use rsubs_lib::{
        SRT,
        SSA,
    };

    use super::{
        clean_subtitle_text,
        parse_srt,
        read_txt,
        sentences_from_lines,
        without_empty_blocks,
    };
    use crate::core::SourceFile;

    #[test]
    fn sentences_from_lines_matches_read_txt() {
        let content = "今日は良い天気。散歩に行く！\n次の行(つぎのぎょう)です。";
        let path = std::env::temp_dir().join(format!("yomine_parity_{}.txt", std::process::id()));
        std::fs::write(&path, content).unwrap();
        let source_file =
            SourceFile { original_file: path.display().to_string(), ..Default::default() };

        let from_file = read_txt(&source_file).unwrap();
        let from_lines = sentences_from_lines(content.lines(), &source_file);
        let _ = std::fs::remove_file(&path);

        let texts: Vec<&str> = from_lines.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["今日は良い天気", "散歩に行く", "次の行です"]);
        assert_eq!(from_file.len(), from_lines.len());
        for (a, b) in from_file.iter().zip(&from_lines) {
            assert_eq!((a.id, &a.text), (b.id, &b.text));
        }
    }

    #[test]
    fn merges_a_line_split_into_touching_cues() {
        // Frieren S01E01: one spoken line split at a scene cut, then said again later.
        let srt = "146\n00:10:37,721 --> 00:10:38,096\n放り込んでおいてくれて\nよかったのに\n\n\
                   147\n00:10:38,096 --> 00:10:40,307\n放り込んでおいてくれて\nよかったのに\n\n\
                   148\n00:10:40,400 --> 00:10:41,000\n次の台詞\n\n\
                   149\n00:10:45,000 --> 00:10:46,000\n次の台詞\n";
        let sentences = parse_srt(SRT::parse(srt).unwrap(), &SourceFile::default()).unwrap();

        let spans: Vec<(usize, (f32, f32))> =
            sentences.iter().map(|s| (s.id, s.timestamp.as_ref().unwrap().to_secs())).collect();
        assert_eq!(sentences.len(), 3);
        assert_eq!(spans[0].0, 0);
        assert!((spans[0].1 .0 - 637.721).abs() < 0.01 && (spans[0].1 .1 - 640.307).abs() < 0.01);
        // A repeat after a real pause is its own line.
        assert_eq!((spans[1].0, spans[2].0), (1, 2));
    }

    #[test]
    fn ass_ending_in_a_blank_line_parses() {
        // The start of a track ffmpeg extracted from an mkv, which ends with a blank line.
        let ass = include_str!("../tests/fixtures/ass/trailing_blank_line.ass");
        assert!(SSA::parse_lenient(ass).is_err());
        assert!(SSA::parse_lenient(without_empty_blocks(ass)).is_ok());
    }

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
