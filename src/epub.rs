use std::collections::HashMap;

use quick_xml::{
    events::Event,
    Reader,
};
use rbook::Epub;

use crate::core::YomineError;

#[derive(Debug, Clone)]
pub struct EpubChapter {
    pub title: String,
    pub char_count: usize,
    pub parts: Vec<EpubPart>,
}

/// One selectable slice of a chapter; `id` is what `chapter_lines` takes back.
#[derive(Debug, Clone)]
pub struct EpubPart {
    pub id: usize,
    pub char_count: usize,
}

const NEGLIGIBLE_CHARS: usize = 10;
/// Target part size when splitting an oversized chapter (~one subtitle file's worth).
const PART_CHARS: usize = 10_000;

struct Section {
    lines: Vec<String>,
    char_count: usize,
}

struct Chapter {
    title: String,
    sections: Vec<Section>,
}

fn open(path: &str) -> Result<Epub, YomineError> {
    Epub::open(path).map_err(|e| YomineError::Custom(format!("Failed to open EPUB: {}", e)))
}

/// Metadata title only — cheap (chapter contents are lazy; nothing is extracted).
pub fn book_title(path: &str) -> Result<String, YomineError> {
    let epub = open(path)?;
    Ok(epub.metadata().title().map(|t| t.value().trim().to_string()).unwrap_or_default())
}

pub fn list_chapters(path: &str) -> Result<(String, Vec<EpubChapter>), YomineError> {
    let epub = open(path)?;
    let book_title =
        epub.metadata().title().map(|t| t.value().trim().to_string()).unwrap_or_default();

    let chapters = book_chapters(&epub);
    if chapters.is_empty() {
        return Err(YomineError::Custom("No readable chapters found in the EPUB.".to_string()));
    }

    let mut id = 0;
    let chapters = chapters
        .into_iter()
        .map(|chapter| EpubChapter {
            title: chapter.title,
            char_count: chapter.sections.iter().map(|s| s.char_count).sum(),
            parts: chapter
                .sections
                .iter()
                .map(|s| {
                    let part = EpubPart { id, char_count: s.char_count };
                    id += 1;
                    part
                })
                .collect(),
        })
        .collect();
    Ok((book_title, chapters))
}

/// `indices` are part ids from `list_chapters` (`None` = every part).
pub fn chapter_lines(path: &str, indices: Option<&[usize]>) -> Result<Vec<String>, YomineError> {
    let epub = open(path)?;
    let mut sections: Vec<Section> =
        book_chapters(&epub).into_iter().flat_map(|c| c.sections).collect();

    let selected: Vec<usize> = match indices {
        Some(indices) => {
            let mut sorted = indices.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            sorted
        }
        None => (0..sections.len()).collect(),
    };

    let mut lines = Vec::new();
    for index in selected {
        if index < sections.len() {
            lines.append(&mut sections[index].lines);
        }
    }
    Ok(lines)
}

/// The deterministic chapter/part table `list_chapters` and `chapter_lines`
/// both address by flattened part position — they must derive identically.
fn book_chapters(epub: &Epub) -> Vec<Chapter> {
    let spine = epub.spine();
    let spine_len = spine.len();

    let mut order_by_href: HashMap<String, usize> = HashMap::new();
    for entry in spine.iter() {
        if let Some(manifest_entry) = entry.manifest_entry() {
            order_by_href.insert(manifest_entry.href().path().as_str().to_string(), entry.order());
        }
    }

    // Each ToC entry starts a chapter spanning spine files up to the next
    // entry — untitled/split files belong to the preceding heading.
    let mut starts: Vec<(usize, String)> = Vec::new();
    if let Some(root) = epub.toc().contents() {
        for entry in root.flatten() {
            let Some(href) = entry.href() else { continue };
            let Some(&position) = order_by_href.get(href.path().as_str()) else { continue };
            starts.push((position, entry.label().trim().to_string()));
        }
    }
    starts.sort_by_key(|&(position, _)| position);
    starts.dedup_by_key(|entry| entry.0);

    let ranges: Vec<(usize, String, std::ops::Range<usize>)> = if starts.is_empty() {
        (0..spine_len).map(|i| (i, String::new(), i..i + 1)).collect()
    } else {
        starts
            .iter()
            .enumerate()
            .map(|(i, (start, title))| {
                let end = starts.get(i + 1).map_or(spine_len, |&(next, _)| next);
                (*start, title.clone(), *start..end)
            })
            .collect()
    };

    let mut chapters = Vec::new();
    for (start, title, range) in ranges {
        let mut lines = Vec::new();
        for index in range {
            let Some(entry) = spine.get(index) else { continue };
            let Some(manifest_entry) = entry.manifest_entry() else { continue };
            let Ok(xhtml) = manifest_entry.read_str() else { continue };
            lines.extend(extract_lines(&xhtml));
        }
        let char_count: usize = lines.iter().map(|l| l.chars().count()).sum();
        if char_count < NEGLIGIBLE_CHARS {
            continue;
        }
        let title = if title.is_empty() { format!("Chapter {}", start + 1) } else { title };
        chapters.push(Chapter { title, sections: split_chapter(lines, char_count) });
    }
    chapters
}

/// Split a chapter into ~`PART_CHARS`-sized parts at paragraph boundaries.
fn split_chapter(lines: Vec<String>, char_count: usize) -> Vec<Section> {
    let parts = ((char_count + PART_CHARS / 2) / PART_CHARS).max(1);
    if parts == 1 {
        return vec![Section { lines, char_count }];
    }

    let mut splits: Vec<Section> = Vec::with_capacity(parts);
    let mut current = Vec::new();
    let mut current_chars = 0;
    let mut consumed = 0;
    for line in lines {
        let chars = line.chars().count();
        current.push(line);
        current_chars += chars;
        consumed += chars;
        // Close a part once it reaches its even share of the chapter.
        if splits.len() + 1 < parts && consumed * parts >= char_count * (splits.len() + 1) {
            splits.push(Section {
                lines: std::mem::take(&mut current),
                char_count: std::mem::take(&mut current_chars),
            });
        }
    }
    if !current.is_empty() {
        splits.push(Section { lines: current, char_count: current_chars });
    }
    splits
}

const SKIPPED_TAGS: &[&[u8]] = &[b"rt", b"rp", b"script", b"style", b"head", b"title"];
const BLOCK_TAGS: &[&[u8]] =
    &[b"p", b"div", b"h1", b"h2", b"h3", b"h4", b"h5", b"h6", b"li", b"br", b"blockquote"];

fn extract_lines(xhtml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xhtml);
    let config = reader.config_mut();
    config.check_end_names = false;
    config.allow_dangling_amp = true;
    config.allow_unmatched_ends = true;

    let mut text = String::new();
    let mut skip_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_ascii_lowercase();
                if SKIPPED_TAGS.contains(&name.as_slice()) {
                    skip_depth += 1;
                } else if BLOCK_TAGS.contains(&name.as_slice()) {
                    text.push('\n');
                }
            }
            Ok(Event::End(e)) => {
                let name = e.local_name().as_ref().to_ascii_lowercase();
                if SKIPPED_TAGS.contains(&name.as_slice()) {
                    skip_depth = skip_depth.saturating_sub(1);
                } else if BLOCK_TAGS.contains(&name.as_slice()) {
                    text.push('\n');
                }
            }
            Ok(Event::Empty(e)) => {
                if e.local_name().as_ref().eq_ignore_ascii_case(b"br") {
                    text.push('\n');
                }
            }
            Ok(Event::Text(e)) => {
                if skip_depth == 0 {
                    if let Ok(t) = e.decode() {
                        text.push_str(&t);
                    }
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if skip_depth == 0 {
                    if let Ok(Some(c)) = e.resolve_char_ref() {
                        text.push(c);
                    } else if let Ok(name) = e.decode() {
                        match name.as_ref() {
                            "lt" => text.push('<'),
                            "gt" => text.push('>'),
                            "amp" => text.push('&'),
                            "quot" => text.push('"'),
                            "apos" => text.push('\''),
                            "nbsp" => text.push(' '),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            // Salvage whatever text was extracted before the malformed markup.
            Err(_) => break,
            Ok(_) => {}
        }
    }

    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn extract_lines_strips_ruby_and_breaks_blocks() {
        let xhtml = "<html><head><title>t</title><style>p{color:red}</style></head><body>\
                     <p><ruby>漢字<rt>かんじ</rt></ruby>を読む。</p>\
                     <p>次の&#12385;段落&nbsp;と<b>強調</b></p>\
                     <div>行A<br/>行B</div></body></html>";
        assert_eq!(extract_lines(xhtml), vec!["漢字を読む。", "次のち段落 と強調", "行A", "行B"]);
    }

    fn fixture_epub(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("yomine_{}_{}.epub", name, std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::SimpleFileOptions::default();

        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", deflated).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#,
        )
        .unwrap();

        zip.start_file("OEBPS/content.opf", deflated).unwrap();
        zip.write_all(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="id" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>吾輩の本</dc:title>
    <dc:identifier id="id">yomine-test</dc:identifier>
    <dc:language>ja</dc:language>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>
    <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c1b" href="c1b.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="cover"/>
    <itemref idref="c1"/>
    <itemref idref="c1b"/>
    <itemref idref="c2"/>
  </spine>
</package>"#
                .as_bytes(),
        )
        .unwrap();

        zip.start_file("OEBPS/toc.ncx", deflated).unwrap();
        zip.write_all(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\">
  <head><meta name=\"dtb:uid\" content=\"yomine-test\"/></head>
  <docTitle><text>吾輩の本</text></docTitle>
  <navMap>
    <navPoint id=\"n1\" playOrder=\"1\"><navLabel><text>第一章</text></navLabel><content src=\"c1.xhtml\"/></navPoint>
    <navPoint id=\"n2\" playOrder=\"2\"><navLabel><text>第二章</text></navLabel><content src=\"c2.xhtml#start\"/></navPoint>
  </navMap>
</ncx>"
                .as_bytes(),
        )
        .unwrap();

        zip.start_file("OEBPS/cover.xhtml", deflated).unwrap();
        zip.write_all(
            b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>Cover</title></head>\
              <body><div><img src=\"cover.jpg\" alt=\"\"/></div></body></html>",
        )
        .unwrap();

        // A title page followed by an untitled body file — the chapter must span both.
        zip.start_file("OEBPS/c1.xhtml", deflated).unwrap();
        zip.write_all(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c1</title></head>\
             <body><p>第一章</p></body></html>"
                .as_bytes(),
        )
        .unwrap();

        zip.start_file("OEBPS/c1b.xhtml", deflated).unwrap();
        zip.write_all(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c1b</title>\
             <style>p { color: red; }</style></head>\
             <body><p><ruby>漢字<rt>かんじ</rt></ruby>を読む。</p>\
             <p>第二段落はこちらです。</p></body></html>"
                .as_bytes(),
        )
        .unwrap();

        zip.start_file("OEBPS/c2.xhtml", deflated).unwrap();
        zip.write_all(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>c2</title></head>\
             <body><h2>見出しです</h2><div>二章の文はここにある。</div></body></html>"
                .as_bytes(),
        )
        .unwrap();

        zip.finish().unwrap();
        path
    }

    #[test]
    fn toc_entries_define_chapters_spanning_untitled_spine_files() {
        let path = fixture_epub("list");
        let (title, chapters) = list_chapters(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(title, "吾輩の本");
        // The cover (spine 0, before the first ToC entry) is dropped; 第一章
        // spans its title page + the untitled body file; 第二章 matches
        // through its fragment href (c2.xhtml#start).
        let summary: Vec<(&str, Vec<usize>)> = chapters
            .iter()
            .map(|c| (c.title.as_str(), c.parts.iter().map(|p| p.id).collect()))
            .collect();
        assert_eq!(summary, vec![("第一章", vec![0]), ("第二章", vec![1])]);
        assert_eq!(
            chapters[0].char_count,
            "第一章漢字を読む。第二段落はこちらです。".chars().count()
        );
    }

    #[test]
    fn split_chapter_makes_even_paragraph_aligned_parts() {
        let lines: Vec<String> = (0..5).map(|_| "あ".repeat(3_000)).collect();
        let sections = split_chapter(lines, 15_000);

        let summary: Vec<(usize, usize)> =
            sections.iter().map(|s| (s.lines.len(), s.char_count)).collect();
        assert_eq!(summary, vec![(3, 9_000), (2, 6_000)]);

        // Small chapters stay whole.
        assert_eq!(split_chapter(vec!["短い。".to_string()], 3).len(), 1);
    }

    #[test]
    fn chapter_lines_strips_ruby_and_honors_selection() {
        let path = fixture_epub("lines");
        let all = chapter_lines(path.to_str().unwrap(), None).unwrap();
        let only_c2 = chapter_lines(path.to_str().unwrap(), Some(&[1])).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            all,
            vec![
                "第一章",
                "漢字を読む。",
                "第二段落はこちらです。",
                "見出しです",
                "二章の文はここにある。"
            ]
        );
        assert_eq!(only_c2, vec!["見出しです", "二章の文はここにある。"]);
    }
}
