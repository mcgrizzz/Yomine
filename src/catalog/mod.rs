//! Which show or film a file is, from TMDB, and which files hold the same episode's text.

pub mod signature;
pub mod store;
pub mod tmdb;

use std::{
    collections::{
        hash_map::Entry,
        HashMap,
    },
    sync::LazyLock,
};

use regex::Regex;
use serde::{
    Deserialize,
    Serialize,
};
use tmdb::{
    Candidate,
    Episode,
    GroupPart,
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

/// A show's seasons as one of its episode groups splits them, for a show TMDB lists as fewer
/// seasons than it aired in: Frieren's second season is its season 1 from episode 29. Matches
/// keep TMDB's numbering and are shown in this one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Each season's number and its episodes' TMDB season and number, in order.
    seasons: Vec<(u32, Vec<(u32, u32)>)>,
}

impl Layout {
    /// The `Season N` parts of an episode group, without its specials.
    fn from_parts(parts: Vec<GroupPart>) -> Option<Self> {
        static SEASON: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)^season\s*(\d+)$").unwrap());
        let mut seasons: Vec<(u32, Vec<(u32, u32)>)> = parts
            .into_iter()
            .filter_map(|mut part| {
                let number = SEASON.captures(part.name.trim())?[1].parse().ok()?;
                part.episodes.sort_by_key(|e| e.order);
                let episodes = part.episodes.iter().map(|e| (e.season_number, e.episode_number));
                (number > 0).then(|| (number, episodes.collect()))
            })
            .collect();
        seasons.sort_by_key(|(number, _)| *number);
        (!seasons.is_empty()).then_some(Layout { seasons })
    }

    /// TMDB's season and number for an episode of this layout. Without a season, or past
    /// the season's end, the number counts on through the seasons, as some releases number.
    pub fn to_tmdb(&self, season: Option<u32>, number: u32) -> Option<(u32, u32)> {
        let index = usize::try_from(number).ok()?.checked_sub(1)?;
        let counted = self.seasons.iter().flat_map(|(_, episodes)| episodes).nth(index).copied();
        let Some(season) = season else { return counted };
        let (_, episodes) = self.seasons.iter().find(|(n, _)| *n == season)?;
        episodes.get(index).copied().or(counted.filter(|e| episodes.contains(e)))
    }

    /// This layout's season and number for TMDB's.
    pub fn from_tmdb(&self, season: u32, number: u32) -> Option<(u32, u32)> {
        self.seasons.iter().find_map(|(n, episodes)| {
            let at = episodes.iter().position(|e| *e == (season, number))?;
            Some((*n, at as u32 + 1))
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
    let (season, number) = match (reference.season, reference.episode) {
        (Some(season), Some(number)) => {
            let found = match layout(tmdb, work.tmdb_id).await? {
                Some(layout) if season > 0 => layout.to_tmdb(Some(season), number),
                _ => None,
            };
            let (season, number) = found.unwrap_or((season, number));
            (Some(season), Some(number))
        }
        numbers => numbers,
    };
    let episode = match (season, number) {
        (Some(season), Some(number)) => episode_in(tmdb, work.tmdb_id, season, number).await?,
        _ => None,
    };
    Ok(Some(Match {
        work,
        season,
        episode: number,
        episode_title: episode.map(|e| e.name).filter(|n| !n.is_empty()),
    }))
}

/// A title's trailing qualifier split off, as in `Wakakozake (Drama)`: the bare title, and
/// whether the qualifier asks for an animated entry or a live-action one.
pub fn qualified(title: &str) -> (&str, Option<bool>) {
    let trimmed = title.trim_end();
    let Some(open) = trimmed.strip_suffix([')', '）']).and_then(|t| t.rfind(['(', '（'])) else {
        return (title, None);
    };
    let bare = trimmed[..open].trim_end();
    if bare.is_empty() {
        return (title, None);
    }
    let qualifier = trimmed[open..].to_lowercase();
    let names = |words: &[&str]| words.iter().any(|w| qualifier.contains(w));
    let animated = if names(&["drama", "dorama", "live action", "live-action", "ドラマ", "実写"])
    {
        Some(false)
    } else if names(&["anime", "animated", "animation", "アニメ"]) {
        Some(true)
    } else {
        None
    };
    (bare, animated)
}

/// Searches TMDB for the title, and for it without a trailing qualifier when TMDB has
/// nothing for the whole. A qualifier naming drama or anime puts those entries first.
pub async fn search_titled(
    tmdb: &Tmdb,
    kind: Kind,
    title: &str,
    year: Option<u32>,
) -> Result<Vec<Candidate>, YomineError> {
    let (bare, animated) = qualified(title);
    let mut found = tmdb.search(kind, title, year).await?;
    if found.is_empty() && bare != title {
        found = tmdb.search(kind, bare, year).await?;
    }
    if let Some(animated) = animated {
        found.sort_by_key(|c| c.animated() != animated);
    }
    Ok(found)
}

/// The search's best result and whether one of its titles is the query's.
async fn search(tmdb: &Tmdb, query: &Query) -> Result<Option<(Work, bool)>, YomineError> {
    let candidates = search_titled(tmdb, query.kind, &query.title, query.year).await?;
    let Some(best) = pick(&candidates) else { return Ok(None) };
    let (bare, animated) = qualified(&query.title);
    if animated.is_some_and(|a| best.animated() != a) {
        return Ok(Some((work_of(query.kind, best), false)));
    }
    let wanted = normalize_title(if animated.is_some() { bare } else { &query.title });
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
    Ok(Some((work_of(query.kind, best), exact)))
}

fn work_of(kind: Kind, candidate: &Candidate) -> Work {
    Work {
        kind,
        tmdb_id: candidate.id,
        title: candidate.title.clone(),
        original_title: candidate.original_title.clone(),
    }
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
    if season != Some(0) {
        if let Some(layout) = layout(tmdb, show).await? {
            let Some((season, number)) = layout.to_tmdb(season, number) else { return Ok(None) };
            return Ok(episode_in(tmdb, show, season, number).await?.map(|e| (season, e)));
        }
    }
    let first = season.unwrap_or(1);
    if let Some(episode) = episode_in(tmdb, show, first, number).await? {
        return Ok(Some((first, episode)));
    }
    if season.is_some() {
        return Ok(None);
    }
    let mut before = 0;
    for (season, count) in regular_seasons(tmdb, show).await? {
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

async fn regular_seasons(tmdb: &Tmdb, show: i64) -> Result<Vec<(u32, u32)>, YomineError> {
    if let Some(seasons) = store::seasons(show)? {
        return Ok(seasons);
    }
    let seasons = tmdb.seasons(show).await?;
    store::set_seasons(show, &seasons)?;
    Ok(seasons)
}

/// The show's layout, where it has an episode group splitting it into more seasons than
/// TMDB lists.
pub async fn layout(tmdb: &Tmdb, show: i64) -> Result<Option<Layout>, YomineError> {
    /// TMDB's Production type, which a show's real seasons are usually grouped under.
    const PRODUCTION: u32 = 6;
    if let Some(cached) = store::layout(show)?.filter(|c| !c.stale) {
        return Ok(cached.layout);
    }
    let regular = regular_seasons(tmdb, show).await?.len();
    let mut groups = tmdb.episode_groups(show).await?;
    groups.retain(|g| g.name.to_lowercase().contains("season"));
    groups.sort_by_key(|g| g.kind != PRODUCTION);
    let mut found = None;
    for group in groups.iter().take(2) {
        let layout = Layout::from_parts(tmdb.episode_group(&group.id).await?);
        if let Some(layout) = layout.filter(|l| l.seasons.len() > regular) {
            found = Some(layout);
            break;
        }
    }
    store::set_layout(show, found.as_ref())?;
    Ok(found)
}

/// The match numbered by the show's layout when one is cached, for showing it.
pub fn displayed(mut found: Match) -> Match {
    let (Kind::Tv, Some(season), Some(number)) = (found.work.kind, found.season, found.episode)
    else {
        return found;
    };
    let (season, number) = displayed_episode(found.work.tmdb_id, season, number);
    found.season = Some(season);
    found.episode = Some(number);
    found
}

/// A TMDB season and episode as [`displayed`] shows them.
pub fn displayed_episode(show: i64, season: u32, number: u32) -> (u32, u32) {
    let layout = store::layout(show).ok().flatten().and_then(|c| c.layout);
    layout.and_then(|l| l.from_tmdb(season, number)).unwrap_or((season, number))
}

/// The episodes of a season, in the show's layout where it has one, that have aired.
pub async fn aired_episodes(
    tmdb: &Tmdb,
    show: i64,
    season: u32,
) -> Result<Vec<Episode>, YomineError> {
    let _one_at_a_time = RESOLVING.lock().await;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let aired = |e: &Episode| e.air_date.as_deref().is_some_and(|d| d <= today.as_str());
    let layout = if season > 0 { layout(tmdb, show).await? } else { None };
    let Some(layout) = layout else {
        return Ok(season_list(tmdb, show, season).await?.into_iter().filter(aired).collect());
    };
    let Some((_, numbers)) = layout.seasons.iter().find(|(n, _)| *n == season) else {
        return Ok(Vec::new());
    };
    let mut lists = HashMap::new();
    let mut episodes = Vec::new();
    for (at, &(tmdb_season, number)) in numbers.iter().enumerate() {
        if let Entry::Vacant(list) = lists.entry(tmdb_season) {
            list.insert(season_list(tmdb, show, tmdb_season).await?);
        }
        let found = lists[&tmdb_season].iter().find(|e| e.number == number);
        if let Some(episode) = found.filter(|e| aired(e)) {
            episodes.push(Episode { number: at as u32 + 1, ..episode.clone() });
        }
    }
    Ok(episodes)
}

/// TMDB's episodes of a season, fetched again once a day for a season that may be airing.
async fn season_list(tmdb: &Tmdb, show: i64, season: u32) -> Result<Vec<Episode>, YomineError> {
    if let Some(cached) = store::season(show, season)?.filter(|c| !c.stale) {
        return Ok(cached.episodes);
    }
    let episodes = tmdb.season(show, season).await?;
    store::set_season(show, season, &episodes)?;
    Ok(episodes)
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
    fn a_layout_numbers_episodes_by_the_groups_seasons() {
        let part = |name: &str, episodes: &[(u32, u32)]| GroupPart {
            name: name.into(),
            order: 0,
            episodes: episodes
                .iter()
                .enumerate()
                .map(|(order, &(season_number, episode_number))| tmdb::GroupEpisode {
                    season_number,
                    episode_number,
                    order: order as u32,
                })
                .collect(),
        };
        // Frieren on TMDB: one season of 38, grouped as 28 and 10 (shortened here).
        let layout = Layout::from_parts(vec![
            part("Specials", &[(0, 1)]),
            part("Season 2", &[(1, 4), (1, 5)]),
            part("Season 1", &[(1, 1), (1, 2), (1, 3)]),
        ])
        .unwrap();
        assert_eq!(layout.to_tmdb(Some(2), 1), Some((1, 4)));
        assert_eq!(layout.to_tmdb(None, 4), Some((1, 4)));
        assert_eq!(layout.to_tmdb(Some(2), 4), Some((1, 4)));
        assert_eq!(layout.to_tmdb(Some(1), 4), None);
        assert_eq!(layout.from_tmdb(1, 5), Some((2, 2)));
        assert_eq!(layout.from_tmdb(0, 1), None);
    }

    #[test]
    fn a_qualifier_asks_for_anime_or_live_action() {
        assert_eq!(qualified("Wakakozake (Drama)"), ("Wakakozake", Some(false)));
        assert_eq!(qualified("ワカコ酒（アニメ）"), ("ワカコ酒", Some(true)));
        assert_eq!(qualified("Mushishi (Live Action)"), ("Mushishi", Some(false)));
        assert_eq!(qualified("Hunter x Hunter (2011)"), ("Hunter x Hunter", None));
        assert_eq!(qualified("Wakakozake"), ("Wakakozake", None));
        assert_eq!(qualified("(Drama)"), ("(Drama)", None));
    }

    #[test]
    fn search_results_and_details_name_their_genres() {
        let searched: tmdb::Candidate =
            serde_json::from_str(r#"{"id":1,"name":"ワカコ酒","genre_ids":[16,35,999]}"#).unwrap();
        assert_eq!(searched.genres, ["Animation", "Comedy"]);
        assert!(searched.animated());
        let details: tmdb::Candidate = serde_json::from_str(
            r#"{"id":2,"name":"ワカコ酒","genres":[{"id":18,"name":"Drama"}]}"#,
        )
        .unwrap();
        assert_eq!(details.genres, ["Drama"]);
        assert!(!details.animated());
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
