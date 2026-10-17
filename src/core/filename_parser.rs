use std::sync::LazyLock;

use anitomy_ng::{
    ElementKind,
    Options,
};
use regex::Regex;

use crate::media::subtitles::strip_language_tag;

#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    TvShow {
        title: String,
        season: Option<u32>,
        episode: Option<u32>,
        source: Option<String>, // e.g., "Netflix", "Crunchyroll"
    },
    Movie {
        title: String,
        year: Option<u32>,
        source: Option<String>,
    },
    Generic {
        title: String,
        source: Option<String>,
    },
}

/// asbplayer names a streaming site's track `<page title> - <track label>`.
static TRACK_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s+-\s+(?:日本語|Japanese)(?:\s*[(\[（][^)\]）]*[)\]）])?$").unwrap()
});
static BRACKETED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]").unwrap());

/// Parses a file name or a path. A path's folders can give the title (`Show/03.mkv`) and
/// the season (`Season 2/`).
pub fn parse_filename(path: &str) -> MediaType {
    let path = fold_fullwidth(path);
    let name_start = path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let name = &path[name_start..];
    let stem = std::path::Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    // asbplayer's extension for YouTube's subtitle format.
    let stem = stem.strip_suffix(".ytsrv3").unwrap_or(stem);
    let stem = strip_language_tag(stem);
    let stem = TRACK_LABEL.replace(stem, "");

    let elements =
        anitomy_ng::parse_path(&format!("{}{stem}", &path[..name_start]), Options::default());
    let first =
        |kind: ElementKind| elements.iter().find(|e| e.kind == kind).map(|e| e.value.as_str());
    let title = first(ElementKind::Title).map(tidy).filter(|t| !t.is_empty());
    let season = first(ElementKind::Season).and_then(|s| s.parse().ok());
    let episode = first(ElementKind::Episode);
    let year = first(ElementKind::Year).and_then(|y| y.parse().ok());
    let source = parse_source(name);

    if episode.is_some() || season.is_some() {
        MediaType::TvShow {
            title: title.unwrap_or_else(|| clean(&stem)),
            season,
            episode: episode.and_then(|e| e.parse().ok()),
            source,
        }
    } else if year.is_some() {
        MediaType::Movie { title: title.unwrap_or_else(|| clean(&stem)), year, source }
    } else {
        // anitomy reads a leading bracket as a release group, which loses 【東京→北京】 from a
        // video's title.
        MediaType::Generic { title: clean(&stem), source }
    }
}

/// anitomy reads ASCII digits only, so `第３話` has no episode until it's `第3話`.
fn fold_fullwidth(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '０'..='９' | 'Ａ'..='Ｚ' | 'ａ'..='ｚ' => {
                char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)
            }
            '\u{3000}' => ' ',
            c => c,
        })
        .collect()
}

fn tidy(title: &str) -> String {
    title.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean(stem: &str) -> String {
    let title = tidy(&BRACKETED.replace_all(stem, "")).trim_end_matches(" -").to_string();
    if title.is_empty() {
        tidy(stem)
    } else {
        title
    }
}

fn parse_source(text: &str) -> Option<String> {
    let sources = vec![
        "Netflix",
        "Crunchyroll",
        "Funimation",
        "Amazon",
        "Hulu",
        "Disney",
        "HBO",
        "Paramount",
        "Apple",
        "Peacock",
        "MAX",
    ];

    for s in sources {
        if text.to_lowercase().contains(&s.to_lowercase()) {
            return Some(s.to_string());
        }
    }

    None
}

impl MediaType {
    pub fn display_title(&self) -> String {
        match self {
            MediaType::TvShow { title, season, episode, .. } => match (season, episode) {
                (Some(s), Some(e)) => format!("{} - S{:02}E{:02}", title, s, e),
                (None, Some(e)) => format!("{} - Episode {}", title, e),
                _ => title.clone(),
            },
            MediaType::Movie { title, year, .. } => match year {
                Some(y) => format!("{} ({})", title, y),
                None => title.clone(),
            },
            MediaType::Generic { title, .. } => title.clone(),
        }
    }

    pub fn get_metadata_string(&self) -> String {
        match self {
            MediaType::TvShow { source, .. } => source.clone().unwrap_or_default(),
            MediaType::Movie { source, .. } => source.clone().unwrap_or_default(),
            MediaType::Generic { source, .. } => source.clone().unwrap_or_default(),
        }
    }

    pub fn is_database_matchable(&self) -> bool {
        match self {
            MediaType::TvShow { season, episode, .. } => season.is_some() && episode.is_some(),
            MediaType::Movie { year, .. } => year.is_some(),
            MediaType::Generic { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_we_have_misread() {
        let show = |title: &str, season: Option<u32>, episode: Option<u32>| MediaType::TvShow {
            title: title.into(),
            season,
            episode,
            source: None,
        };
        let cases = [
            // A checksum's `E4` isn't an episode.
            (
                "[SubsPlease] Mushoku Tensei - 01v2 (1080p) [F76E4E71].ja.srt",
                show("Mushoku Tensei", None, Some(1)),
            ),
            ("Steins;Gate 0 - 12 [BD 1080p].mkv", show("Steins;Gate 0", None, Some(12))),
            ("One Piece 1071.srt", show("One Piece", None, Some(1071))),
            ("葬送のフリーレン　第３話.srt", show("葬送のフリーレン", None, Some(3))),
            ("/anime/葬送のフリーレン/Season 2/03.mkv", show("葬送のフリーレン", Some(2), Some(3))),
            (
                "Frieren.Beyond.Journeys.End.S01E01.1080p.WEBRip.x265.DDP.2.0-ZeroBuild.jpn.forced.srt",
                show("Frieren Beyond Journeys End", Some(1), Some(1)),
            ),
            (
                "【東京→北京】JAL国際線ビジネスクラスに初搭乗 - 日本語 (自動生成).srt",
                MediaType::Generic {
                    title: "【東京→北京】JAL国際線ビジネスクラスに初搭乗".into(),
                    source: None,
                },
            ),
        ];
        for (name, expected) in cases {
            assert_eq!(parse_filename(name), expected, "{name}");
        }
    }

    #[test]
    fn test_media_type_methods() {
        // Test display_title method
        let tv_show = MediaType::TvShow {
            title: "Attack On Titan".to_string(),
            season: Some(1),
            episode: Some(5),
            source: None,
        };
        assert_eq!(tv_show.display_title(), "Attack On Titan - S01E05");

        let movie =
            MediaType::Movie { title: "Spirited Away".to_string(), year: Some(2001), source: None };
        assert_eq!(movie.display_title(), "Spirited Away (2001)");

        // Test database_matchable method
        assert!(tv_show.is_database_matchable()); // Has season and episode
        assert!(movie.is_database_matchable()); // Has year

        let generic = MediaType::Generic { title: "Generic".to_string(), source: None };
        assert!(!generic.is_database_matchable()); // Generic types are never matchable
    }
}
