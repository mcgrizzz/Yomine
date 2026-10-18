//! Which show or film a file is, from TMDB, and which files hold the same episode's text.

pub mod signature;
pub mod store;
pub mod tmdb;

use std::sync::LazyLock;

use regex::Regex;
use serde::{
    Deserialize,
    Serialize,
};
use tmdb::{
    Candidate,
    Episode,
    Tmdb,
};
use unicode_normalization::UnicodeNormalization;

use crate::core::{
    filename_parser::{
        parse_filename,
        MediaType,
    },
    YomineError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Tv,
    Movie,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Tv => "tv",
            Kind::Movie => "movie",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "tv" => Some(Kind::Tv),
            "movie" => Some(Kind::Movie),
            _ => None,
        }
    }
}

/// A show or film as TMDB lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work {
    pub kind: Kind,
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Match {
    pub work: Work,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub episode_title: Option<String>,
}

/// What a file's name says it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub kind: Kind,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

impl Query {
    /// A show's episode or a film; other names aren't looked up.
    pub fn from_name(name: &str) -> Option<Self> {
        match parse_filename(name) {
            MediaType::TvShow { title, season, episode: Some(episode), .. } => {
                Some(Query { kind: Kind::Tv, title, year: None, season, episode: Some(episode) })
            }
            MediaType::Movie { title, year: Some(year), .. } => Some(Query {
                kind: Kind::Movie,
                title,
                year: Some(year),
                season: None,
                episode: None,
            }),
            _ => None,
        }
    }

    /// What searches and corrections are kept under.
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.kind.as_str(),
            normalize_title(&self.title),
            self.year.map(|y| y.to_string()).unwrap_or_default()
        )
    }
}

/// A TMDB entry the user named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub kind: Kind,
    pub tmdb_id: i64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

impl Reference {
    /// A TMDB link (`themoviedb.org/tv/55925-hanzawa-naoki/season/2/episode/3`), or a bare ID
    /// of the given kind.
    pub fn parse(text: &str, kind: Kind) -> Option<Self> {
        static LINK: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"themoviedb\.org/(tv|movie)/(\d+)[^/?#]*(?:/season/(\d+)(?:/episode/(\d+))?)?",
            )
            .unwrap()
        });
        let text = text.trim();
        if let Ok(tmdb_id) = text.parse() {
            return Some(Reference { kind, tmdb_id, season: None, episode: None });
        }
        let link = LINK.captures(text)?;
        let number = |i: usize| link.get(i).and_then(|m| m.as_str().parse().ok());
        Some(Reference {
            kind: Kind::from_key(&link[1])?,
            tmdb_id: link[2].parse().ok()?,
            season: number(3),
            episode: number(4),
        })
    }
}

pub struct Resolved {
    pub found: Match,
    /// The title matched one of the work's titles exactly and, for a show, TMDB has the
    /// episode. Anything less is only a suggestion.
    pub confident: bool,
}

static RESOLVING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The query's work and episode, from the cache where it has them. Lookups run one at a
/// time, so a queue of one show's episodes searches for the show once.
pub async fn resolve(tmdb: &Tmdb, query: &Query) -> Result<Option<Resolved>, YomineError> {
    let _one_at_a_time = RESOLVING.lock().await;
    let key = query.key();
    let (work, exact) = match store::correction(&key)? {
        Some(None) => return Ok(None),
        Some(Some(work)) => (work, true),
        None => match store::title(&key)? {
            Some(store::CachedTitle { work: None, .. }) => return Ok(None),
            Some(store::CachedTitle { work: Some(work), exact }) => (work, exact),
            None => {
                let found = search(tmdb, query).await?;
                store::set_title(&key, query.kind, found.as_ref().map(|(w, _)| w), {
                    found.as_ref().is_some_and(|(_, exact)| *exact)
                })?;
                match found {
                    Some(found) => found,
                    None => return Ok(None),
                }
            }
        },
    };
    let episode = match (query.kind, query.episode) {
        (Kind::Tv, Some(number)) => {
            match store::season_correction(&key, query.season.unwrap_or(0))? {
                Some((season, offset)) => match u32::try_from(i64::from(number) + offset) {
                    Ok(number) => {
                        episode_in(tmdb, work.tmdb_id, season, number).await?.map(|e| (season, e))
                    }
                    Err(_) => None,
                },
                None => find_episode(tmdb, work.tmdb_id, query.season, number).await?,
            }
        }
        _ => None,
    };
    let confident = exact && (query.kind == Kind::Movie || episode.is_some());
    let found = match episode {
        Some((season, episode)) => Match {
            work,
            season: Some(season),
            episode: Some(episode.number),
            episode_title: Some(episode.name).filter(|n| !n.is_empty()),
        },
        None => Match { work, season: query.season, episode: query.episode, episode_title: None },
    };
    Ok(Some(Resolved { found, confident }))
}

/// Keeps the user's match for the query's title and, for a show, where its season numbering
/// lands on TMDB, so the title's other files follow it.
pub fn remember(query: &Query, found: &Match) -> Result<(), YomineError> {
    let key = query.key();
    store::set_correction(&key, found.work.kind, Some(&found.work))?;
    if let (Some(number), Some(season), Some(episode)) =
        (query.episode, found.season, found.episode)
    {
        let offset = i64::from(episode) - i64::from(number);
        store::set_season_correction(&key, query.season.unwrap_or(0), season, offset)?;
    }
    Ok(())
}

/// Keeps the user's answer that the query's title isn't a show or film.
pub fn forget(query: &Query) -> Result<(), YomineError> {
    store::set_correction(&query.key(), query.kind, None)
}

/// The match a reference names, with TMDB's titles. The user's season and episode stand
/// even where TMDB doesn't list them.
pub async fn lookup(tmdb: &Tmdb, reference: &Reference) -> Result<Option<Match>, YomineError> {
    let Some(details) = tmdb.details(reference.kind, reference.tmdb_id).await? else {
        return Ok(None);
    };
    let work = Work {
        kind: reference.kind,
        tmdb_id: reference.tmdb_id,
        title: details.title,
        original_title: details.original_title,
    };
    if work.kind == Kind::Movie {
        return Ok(Some(Match { work, season: None, episode: None, episode_title: None }));
    }
    let episode = match (reference.season, reference.episode) {
        (Some(season), Some(number)) => episode_in(tmdb, work.tmdb_id, season, number).await?,
        _ => None,
    };
    Ok(Some(Match {
        work,
        season: reference.season,
        episode: reference.episode,
        episode_title: episode.map(|e| e.name).filter(|n| !n.is_empty()),
    }))
}

/// The search's best result and whether one of its titles is the query's.
async fn search(tmdb: &Tmdb, query: &Query) -> Result<Option<(Work, bool)>, YomineError> {
    let candidates = tmdb.search(query.kind, &query.title, query.year).await?;
    let Some(best) = pick(&candidates) else { return Ok(None) };
    let wanted = normalize_title(&query.title);
    let mut exact =
        [&best.title, &best.original_title].iter().any(|t| normalize_title(t) == wanted);
    // Romaji titles from release names are usually among the alternative titles.
    if !exact {
        exact = tmdb
            .alternative_titles(query.kind, best.id)
            .await?
            .iter()
            .any(|t| normalize_title(t) == wanted);
    }
    let work = Work {
        kind: query.kind,
        tmdb_id: best.id,
        title: best.title.clone(),
        original_title: best.original_title.clone(),
    };
    Ok(Some((work, exact)))
}

/// The first Japanese-language result, otherwise the first: `silent` is the 2022 drama rather
/// than the more popular Silent Witness.
fn pick(candidates: &[Candidate]) -> Option<&Candidate> {
    candidates.iter().find(|c| c.original_language == "ja").or(candidates.first())
}

/// The season and episode TMDB lists for a file's numbers. Without a season, a number past
/// the first season counts on through the later ones.
async fn find_episode(
    tmdb: &Tmdb,
    show: i64,
    season: Option<u32>,
    number: u32,
) -> Result<Option<(u32, Episode)>, YomineError> {
    let first = season.unwrap_or(1);
    if let Some(episode) = episode_in(tmdb, show, first, number).await? {
        return Ok(Some((first, episode)));
    }
    if season.is_some() {
        return Ok(None);
    }
    let seasons = match store::seasons(show)? {
        Some(seasons) => seasons,
        None => {
            let seasons = tmdb.seasons(show).await?;
            store::set_seasons(show, &seasons)?;
            seasons
        }
    };
    let mut before = 0;
    for (season, count) in seasons {
        if number <= before + count {
            // Some long shows keep counting across seasons, as One Piece does on TMDB.
            let episode = match episode_in(tmdb, show, season, number).await? {
                Some(episode) => Some(episode),
                None => episode_in(tmdb, show, season, number - before).await?,
            };
            return Ok(episode.map(|e| (season, e)));
        }
        before += count;
    }
    Ok(None)
}

async fn episode_in(
    tmdb: &Tmdb,
    show: i64,
    season: u32,
    number: u32,
) -> Result<Option<Episode>, YomineError> {
    let find = |episodes: Vec<Episode>| episodes.into_iter().find(|e| e.number == number);
    if let Some(cached) = store::season(show, season)? {
        let stale = cached.stale;
        match find(cached.episodes) {
            Some(episode) => return Ok(Some(episode)),
            None if !stale => return Ok(None),
            None => {}
        }
    }
    let episodes = tmdb.season(show, season).await?;
    store::set_season(show, season, &episodes)?;
    Ok(find(episodes))
}

/// A title reduced to what two spellings of it share: width, case, accents and punctuation
/// are dropped.
pub fn normalize_title(title: &str) -> String {
    let folded: String = title
        .nfkc()
        // Only Latin letters lose their accents: decomposing kana would turn が into か.
        .map(|c| if ('\u{C0}'..='\u{24F}').contains(&c) { c.nfd().next().unwrap_or(c) } else { c })
        .collect::<String>()
        .to_lowercase();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tmdb_links_and_bare_ids() {
        let reference =
            |kind, tmdb_id, season, episode| Reference { kind, tmdb_id, season, episode };
        assert_eq!(
            Reference::parse(
                "https://www.themoviedb.org/tv/55925-hanzawa-naoki/season/2/episode/3?language=ja",
                Kind::Movie
            ),
            Some(reference(Kind::Tv, 55925, Some(2), Some(3)))
        );
        assert_eq!(
            Reference::parse("themoviedb.org/movie/129-spirited-away", Kind::Tv),
            Some(reference(Kind::Movie, 129, None, None))
        );
        assert_eq!(
            Reference::parse(" 209867 ", Kind::Tv),
            Some(reference(Kind::Tv, 209867, None, None))
        );
        assert_eq!(Reference::parse("Frieren", Kind::Tv), None);
    }

    #[test]
    fn titles_compare_without_width_accents_or_punctuation() {
        assert_eq!(
            normalize_title("Stone Hashira: Gyōmei Himejima"),
            "stone hashira gyomei himejima"
        );
        assert_eq!(normalize_title("ＳＰＹ×ＦＡＭＩＬＹ"), "spy family");
        assert_eq!(normalize_title("ダンジョン飯"), "ダンジョン飯");
    }
}
