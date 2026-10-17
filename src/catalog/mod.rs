//! Which show or film a file is, from TMDB, and which files hold the same episode's text.

pub mod signature;
pub mod store;
pub mod tmdb;

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
    let key = format!(
        "{}:{}:{}",
        query.kind.as_str(),
        normalize_title(&query.title),
        query.year.map(|y| y.to_string()).unwrap_or_default()
    );
    let (work, exact) = match store::title(&key)? {
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
    };
    let episode = match (query.kind, query.episode) {
        (Kind::Tv, Some(number)) => find_episode(tmdb, work.tmdb_id, query.season, number).await?,
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
    fn titles_compare_without_width_accents_or_punctuation() {
        assert_eq!(
            normalize_title("Stone Hashira: Gyōmei Himejima"),
            "stone hashira gyomei himejima"
        );
        assert_eq!(normalize_title("ＳＰＹ×ＦＡＭＩＬＹ"), "spy family");
        assert_eq!(normalize_title("ダンジョン飯"), "ダンジョン飯");
    }
}
