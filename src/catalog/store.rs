//! TMDB's answers kept on disk and shared by every profile, so a show is searched once.

use std::sync::Mutex;

use rusqlite::{
    params,
    Connection,
    OptionalExtension,
};

use super::{
    tmdb::Episode,
    Kind,
    Work,
};
use crate::{
    core::YomineError,
    persistence::get_shared_file_path,
};

const FILE: &str = "catalog.db";
const DAY_MS: i64 = 86_400_000;
/// TMDB's terms allow keeping its data for six months.
const KEPT_MS: i64 = 180 * DAY_MS;
/// A title TMDB had nothing for is searched again after this, in case it was added.
const NOT_FOUND_KEPT_MS: i64 = 7 * DAY_MS;
/// A season without a file's episode may still be airing.
const SEASON_STALE_MS: i64 = DAY_MS;

/// Only a cache: a different `VERSION` drops the tables rather than migrating them.
const VERSION: i64 = 1;
const SCHEMA: &str = "
DROP TABLE IF EXISTS titles;
DROP TABLE IF EXISTS seasons;
DROP TABLE IF EXISTS shows;
-- A searched title, keyed by kind, normalized title and year.
CREATE TABLE titles (
    key TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    -- NULL when TMDB had no result.
    tmdb_id INTEGER,
    title TEXT,
    original_title TEXT,
    exact INTEGER NOT NULL,
    fetched_at INTEGER NOT NULL
);
CREATE TABLE seasons (
    show_id INTEGER NOT NULL,
    season INTEGER NOT NULL,
    -- JSON: [{number, name}].
    episodes TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (show_id, season)
);
CREATE TABLE shows (
    show_id INTEGER PRIMARY KEY,
    -- JSON: [[season, episode_count]].
    seasons TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);
";

static DB: Mutex<Option<Connection>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, YomineError> {
    let error = |e: rusqlite::Error| YomineError::Custom(format!("Catalog cache: {e}"));
    let mut guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(open(&get_shared_file_path(FILE)).map_err(error)?);
    }
    f(guard.as_ref().expect("opened above")).map_err(error)
}

fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != VERSION {
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", VERSION)?;
    }
    Ok(conn)
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub struct CachedTitle {
    /// `None` when TMDB had no result.
    pub work: Option<Work>,
    pub exact: bool,
}

pub fn title(key: &str) -> Result<Option<CachedTitle>, YomineError> {
    with(|conn| {
        conn.query_row(
            "SELECT kind, tmdb_id, title, original_title, exact, fetched_at FROM titles
             WHERE key = ?1",
            params![key],
            |r| {
                let id: Option<i64> = r.get(1)?;
                let fetched_at: i64 = r.get(5)?;
                let kept = if id.is_some() { KEPT_MS } else { NOT_FOUND_KEPT_MS };
                if now() - fetched_at > kept {
                    return Ok(None);
                }
                let work = match (id, Kind::from_key(&r.get::<_, String>(0)?)) {
                    (Some(tmdb_id), Some(kind)) => Some(Work {
                        kind,
                        tmdb_id,
                        title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        original_title: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    }),
                    _ => None,
                };
                Ok(Some(CachedTitle { work, exact: r.get(4)? }))
            },
        )
        .optional()
        .map(Option::flatten)
    })
}

pub fn set_title(
    key: &str,
    kind: Kind,
    work: Option<&Work>,
    exact: bool,
) -> Result<(), YomineError> {
    with(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO titles
                 (key, kind, tmdb_id, title, original_title, exact, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                key,
                kind.as_str(),
                work.map(|w| w.tmdb_id),
                work.map(|w| w.title.as_str()),
                work.map(|w| w.original_title.as_str()),
                exact,
                now()
            ],
        )
        .map(|_| ())
    })
}

pub struct CachedSeason {
    pub episodes: Vec<Episode>,
    pub stale: bool,
}

pub fn season(show: i64, season: u32) -> Result<Option<CachedSeason>, YomineError> {
    let row = with(|conn| {
        conn.query_row(
            "SELECT episodes, fetched_at FROM seasons WHERE show_id = ?1 AND season = ?2",
            params![show, season],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()
    })?;
    let Some((episodes, fetched_at)) = row else { return Ok(None) };
    let age = now() - fetched_at;
    if age > KEPT_MS {
        return Ok(None);
    }
    Ok(Some(CachedSeason {
        episodes: serde_json::from_str(&episodes)?,
        stale: age > SEASON_STALE_MS,
    }))
}

pub fn set_season(show: i64, season: u32, episodes: &[Episode]) -> Result<(), YomineError> {
    let episodes = serde_json::to_string(episodes)?;
    with(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO seasons (show_id, season, episodes, fetched_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![show, season, episodes, now()],
        )
        .map(|_| ())
    })
}

pub fn seasons(show: i64) -> Result<Option<Vec<(u32, u32)>>, YomineError> {
    let row = with(|conn| {
        conn.query_row(
            "SELECT seasons, fetched_at FROM shows WHERE show_id = ?1",
            params![show],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()
    })?;
    match row {
        // A show still airing adds seasons.
        Some((seasons, fetched_at)) if now() - fetched_at <= SEASON_STALE_MS => {
            Ok(Some(serde_json::from_str(&seasons)?))
        }
        _ => Ok(None),
    }
}

pub fn set_seasons(show: i64, seasons: &[(u32, u32)]) -> Result<(), YomineError> {
    let seasons = serde_json::to_string(seasons)?;
    with(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO shows (show_id, seasons, fetched_at) VALUES (?1, ?2, ?3)",
            params![show, seasons, now()],
        )
        .map(|_| ())
    })
}
