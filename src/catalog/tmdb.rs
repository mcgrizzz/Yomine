//! The TMDB endpoints matching needs: <https://developer.themoviedb.org/reference>.

use std::time::Duration;

use reqwest::{
    header::RETRY_AFTER,
    StatusCode,
};
use serde::{
    de::DeserializeOwned,
    Deserialize,
    Serialize,
};

use super::Kind;
use crate::core::YomineError;

const API: &str = "https://api.themoviedb.org/3";

/// A user's own key, otherwise the one release builds embed from CI.
pub fn api_key(user: Option<&str>) -> Option<String> {
    user.map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .or_else(|| option_env!("YOMINE_TMDB_KEY").map(str::to_string))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "RawCandidate")]
pub struct Candidate {
    pub id: i64,
    pub title: String,
    pub original_title: String,
    pub original_language: String,
    pub release_date: String,
    /// What tells a show's anime from its live-action drama, which share a title.
    pub genres: Vec<String>,
}

impl Candidate {
    pub fn animated(&self) -> bool {
        self.genres.iter().any(|g| g == "Animation")
    }
}

#[derive(Deserialize)]
struct RawCandidate {
    id: i64,
    #[serde(default, alias = "name")]
    title: String,
    #[serde(default, alias = "original_name")]
    original_title: String,
    #[serde(default)]
    original_language: String,
    #[serde(default, alias = "first_air_date")]
    release_date: String,
    /// A search result's genres.
    #[serde(default)]
    genre_ids: Vec<i64>,
    /// An entry's details' genres.
    #[serde(default)]
    genres: Vec<Genre>,
}

#[derive(Deserialize)]
struct Genre {
    name: String,
}

impl From<RawCandidate> for Candidate {
    fn from(raw: RawCandidate) -> Self {
        let genres = if raw.genres.is_empty() {
            raw.genre_ids.into_iter().filter_map(genre_name).map(str::to_string).collect()
        } else {
            raw.genres.into_iter().map(|g| g.name).collect()
        };
        Candidate {
            id: raw.id,
            title: raw.title,
            original_title: raw.original_title,
            original_language: raw.original_language,
            release_date: raw.release_date,
            genres,
        }
    }
}

/// The names of TMDB's film and TV genres (`/genre/movie/list`, `/genre/tv/list`), which
/// search results give only as IDs.
fn genre_name(id: i64) -> Option<&'static str> {
    Some(match id {
        12 => "Adventure",
        14 => "Fantasy",
        16 => "Animation",
        18 => "Drama",
        27 => "Horror",
        28 => "Action",
        35 => "Comedy",
        36 => "History",
        37 => "Western",
        53 => "Thriller",
        80 => "Crime",
        99 => "Documentary",
        878 => "Science Fiction",
        9648 => "Mystery",
        10402 => "Music",
        10749 => "Romance",
        10751 => "Family",
        10752 => "War",
        10759 => "Action & Adventure",
        10762 => "Kids",
        10763 => "News",
        10764 => "Reality",
        10765 => "Sci-Fi & Fantasy",
        10766 => "Soap",
        10767 => "Talk",
        10768 => "War & Politics",
        10770 => "TV Movie",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    #[serde(alias = "episode_number")]
    pub number: u32,
    #[serde(default)]
    pub name: String,
    /// `YYYY-MM-DD`; TMDB lists episodes before they air, some without a date.
    #[serde(default)]
    pub air_date: Option<String>,
}

/// One of a show's episode groups: another split of its episodes, which contributors add.
#[derive(Debug, Deserialize)]
pub struct EpisodeGroup {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u32,
}

/// A group's part, like `Season 2`, with its episodes' TMDB season and number, in order.
#[derive(Debug, Deserialize)]
pub struct GroupPart {
    pub name: String,
    pub order: u32,
    pub episodes: Vec<GroupEpisode>,
}

#[derive(Debug, Deserialize)]
pub struct GroupEpisode {
    pub season_number: u32,
    pub episode_number: u32,
    pub order: u32,
}

pub struct Tmdb {
    client: reqwest::Client,
    key: String,
}

impl Tmdb {
    pub fn new(key: String) -> Self {
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap_or_default();
        Self { client, key }
    }

    /// `None` when TMDB has no such entry.
    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Option<T>, YomineError> {
        for attempt in 0..2 {
            let response = self
                .client
                .get(format!("{API}{path}"))
                .query(&[("api_key", self.key.as_str())])
                .query(query)
                .send()
                .await?;
            match response.status() {
                StatusCode::NOT_FOUND => return Ok(None),
                StatusCode::TOO_MANY_REQUESTS if attempt == 0 => {
                    let wait = response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(2);
                    tokio::time::sleep(Duration::from_secs(wait.min(10))).await;
                }
                _ => return Ok(Some(response.error_for_status()?.json().await?)),
            }
        }
        Err(YomineError::Custom("TMDB is limiting requests; try again later".into()))
    }

    pub async fn search(
        &self,
        kind: Kind,
        query: &str,
        year: Option<u32>,
    ) -> Result<Vec<Candidate>, YomineError> {
        #[derive(Deserialize)]
        struct Results {
            results: Vec<Candidate>,
        }
        let mut params = vec![("query", query.to_string()), ("include_adult", "false".into())];
        if let Some(year) = year {
            params.push(("year", year.to_string()));
        }
        let path = format!("/search/{}", kind.as_str());
        Ok(self.get::<Results>(&path, &params).await?.map_or_else(Vec::new, |r| r.results))
    }

    pub async fn details(&self, kind: Kind, id: i64) -> Result<Option<Candidate>, YomineError> {
        self.get(&format!("/{}/{id}", kind.as_str()), &[]).await
    }

    pub async fn alternative_titles(
        &self,
        kind: Kind,
        id: i64,
    ) -> Result<Vec<String>, YomineError> {
        #[derive(Deserialize)]
        struct Titles {
            // A film's list is `titles`, a show's `results`.
            #[serde(alias = "titles")]
            results: Vec<Title>,
        }
        #[derive(Deserialize)]
        struct Title {
            title: String,
        }
        let path = format!("/{}/{id}/alternative_titles", kind.as_str());
        let titles = self.get::<Titles>(&path, &[]).await?;
        Ok(titles.map_or_else(Vec::new, |t| t.results.into_iter().map(|t| t.title).collect()))
    }

    /// Each season's number and episode count, in order, without season 0's specials.
    pub async fn seasons(&self, show: i64) -> Result<Vec<(u32, u32)>, YomineError> {
        #[derive(Deserialize)]
        struct Show {
            seasons: Vec<Season>,
        }
        #[derive(Deserialize)]
        struct Season {
            season_number: u32,
            episode_count: u32,
        }
        let show = self.get::<Show>(&format!("/tv/{show}"), &[]).await?;
        let mut seasons: Vec<(u32, u32)> = show
            .map_or_else(Vec::new, |s| s.seasons)
            .into_iter()
            .filter(|s| s.season_number > 0)
            .map(|s| (s.season_number, s.episode_count))
            .collect();
        seasons.sort();
        Ok(seasons)
    }

    pub async fn episode_groups(&self, show: i64) -> Result<Vec<EpisodeGroup>, YomineError> {
        #[derive(Deserialize)]
        struct Groups {
            results: Vec<EpisodeGroup>,
        }
        let path = format!("/tv/{show}/episode_groups");
        Ok(self.get::<Groups>(&path, &[]).await?.map_or_else(Vec::new, |g| g.results))
    }

    pub async fn episode_group(&self, id: &str) -> Result<Vec<GroupPart>, YomineError> {
        #[derive(Deserialize)]
        struct Group {
            groups: Vec<GroupPart>,
        }
        let path = format!("/tv/episode_group/{id}");
        Ok(self.get::<Group>(&path, &[]).await?.map_or_else(Vec::new, |g| g.groups))
    }

    pub async fn season(&self, show: i64, season: u32) -> Result<Vec<Episode>, YomineError> {
        #[derive(Deserialize)]
        struct Season {
            episodes: Vec<Episode>,
        }
        let season = self.get::<Season>(&format!("/tv/{show}/season/{season}"), &[]).await?;
        Ok(season.map_or_else(Vec::new, |s| s.episodes))
    }
}
