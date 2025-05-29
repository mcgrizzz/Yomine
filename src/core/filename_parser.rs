use regex::Regex;

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

pub fn parse_filename(filename: &str) -> MediaType {
    let stem =
        std::path::Path::new(filename).file_stem().and_then(|s| s.to_str()).unwrap_or(filename);

    if let Some(tv_info) = parse_tv_show(stem) {
        return tv_info;
    }

    if let Some(movie_info) = parse_movie(stem) {
        return movie_info;
    }

    parse_generic(stem)
}

fn parse_tv_show(filename: &str) -> Option<MediaType> {
    let patterns = vec![
        // "Show.Name.S01E02.1080p.mkv"
        r"(?i)(.+?)[\.\s\-\_\[\]]*[Ss](\d{1,2})[Ee](\d{1,3})(?:[\.\s\-\_]*(.+))?",
        // "Show Name episode 12"
        r"(?i)(.+?)[\.\s\-\_\[\]]*(?:episode|ep)[\.\s\-\_]*(\d{1,3})(?:[\.\s\-\_]*(.+))?",
        // "Show Name 12 (Special)" Non season
        r"(?i)(.+?)[\s\-]+(\d{1,3})(?:\s+\((.+?)\)|\s+(.+?))?$",
    ];

    for (i, pattern) in patterns.iter().enumerate() {
        let re = Regex::new(pattern).ok()?;
        if let Some(captures) = re.captures(filename) {
            let title_str = captures.get(1)?.as_str();
            let title = clean_title(title_str);

            let (season, episode) = match i {
                0 => {
                    let season = captures.get(2)?.as_str().parse().ok()?;
                    let episode = captures.get(3)?.as_str().parse().ok()?;
                    (Some(season), Some(episode))
                }
                1 => {
                    let episode = captures.get(2)?.as_str().parse().ok()?;
                    (None, Some(episode))
                }
                2 => {
                    let episode_str = captures.get(2)?.as_str();
                    let episode: u32 = episode_str.parse().ok()?;

                    if episode > 200 {
                        continue;
                    }

                    let remaining = captures
                        .get(3)
                        .or_else(|| captures.get(4))
                        .map(|m| m.as_str())
                        .unwrap_or("");
                    if remaining.contains("x") && remaining.contains(episode_str) {
                        continue;
                    }

                    (None, Some(episode))
                }
                _ => {
                    continue;
                }
            };

            let remaining = captures.get(captures.len() - 1).map(|m| m.as_str()).unwrap_or("");
            let source = parse_source(remaining).or_else(|| parse_source(filename));

            return Some(MediaType::TvShow { title, season, episode, source });
        }
    }

    None
}

fn parse_movie(filename: &str) -> Option<MediaType> {
    let year_pattern = r"(?i)(.+?)[\s\.\-\_]*\((\d{4})\)(?:[\s\.\-\_]*(.+))?";
    let re = Regex::new(year_pattern).ok()?;

    if let Some(captures) = re.captures(filename) {
        let title = clean_title(captures.get(1)?.as_str());
        let year = captures.get(2)?.as_str().parse().ok();
        let remaining = captures.get(3).map(|m| m.as_str()).unwrap_or("");
        let source = parse_source(remaining);

        return Some(MediaType::Movie { title, year, source });
    }

    None
}

fn parse_generic(filename: &str) -> MediaType {
    let title = clean_title(filename);
    let source = parse_source(filename);

    MediaType::Generic { title, source }
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

fn clean_title(title: &str) -> String {
    let mut cleaned = title.to_string();

    let bracket_re = Regex::new(r"[\[\(][^\]\)]*[\]\)]").unwrap();
    cleaned = bracket_re.replace_all(&cleaned, "").to_string();

    let lang_re = Regex::new(r"\.(?:ja|en|es|fr|de|it|pt|ru|ko|zh)(?:\[cc\])?$").unwrap();
    cleaned = lang_re.replace_all(&cleaned, "").to_string();

    cleaned = cleaned.replace(['.', '_', '-'], " ");

    let space_re = Regex::new(r"\s+").unwrap();
    cleaned = space_re.replace_all(&cleaned, " ").trim().to_string();

    let alphabetic_chars: Vec<char> = cleaned.chars().filter(|c| c.is_alphabetic()).collect();
    if alphabetic_chars.is_empty() {
        return cleaned;
    }

    let is_all_upper = alphabetic_chars.iter().all(|c| c.is_uppercase());
    let is_all_lower = alphabetic_chars.iter().all(|c| c.is_lowercase());

    if is_all_upper || is_all_lower {
        cleaned
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>()
                            + chars.as_str().to_lowercase().as_str()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        cleaned
    }
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
    fn test_filename_parsing() {
        // Test TV show parsing with Japanese title, source detection, and language suffix
        let tv_result = parse_filename(
            "ダンダダン.S01E08.なんかモヤモヤするじゃんよ.WEBRip.Netflix.ja[cc].srt",
        );
        if let MediaType::TvShow { title, season, episode, source } = tv_result {
            assert_eq!(title, "ダンダダン");
            assert_eq!(season, Some(1));
            assert_eq!(episode, Some(8));
            assert_eq!(source, Some("Netflix".to_string()));
        } else {
            panic!("Expected TvShow, got {:?}", tv_result);
        }

        // Test movie parsing
        let movie_result = parse_filename("Your Name (2016) [1080p] BluRay.mkv");
        if let MediaType::Movie { title, year, source } = movie_result {
            assert_eq!(title, "Your Name");
            assert_eq!(year, Some(2016));
            assert!(source.is_none());
        } else {
            panic!("Expected Movie, got {:?}", movie_result);
        }

        // Test generic parsing
        let generic_result = parse_filename(
            "[Japanese] 【九州→東京】豪華フェリーの高級個室に乗船 [DownSub.com].srt",
        );
        if let MediaType::Generic { title, source } = generic_result {
            assert_eq!(title, "【九州→東京】豪華フェリーの高級個室に乗船");
            assert!(source.is_none());
        } else {
            panic!("Expected Generic, got {:?}", generic_result);
        }
    }

    #[test]
    fn test_title_cleaning() {
        // Test various title cleaning scenarios
        assert_eq!(clean_title("[Release] show.name_here"), "Show Name Here");
        assert_eq!(clean_title("SHOW_NAME"), "Show Name");
        assert_eq!(clean_title("show.name.ja"), "Show Name"); // Language suffix removal
        assert_eq!(clean_title("movie.title.en[cc]"), "Movie Title");
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
