//! Matching a loaded file to its show or film after the load, without holding it up, and
//! the user's corrections.

use std::{
    collections::HashMap,
    sync::Mutex,
};

use serde::Serialize;
use tauri::{
    AppHandle,
    Emitter,
    Manager,
};
use yomine::{
    catalog::{
        self,
        signature::Signature,
        tmdb::{
            self,
            Candidate,
            Tmdb,
        },
        Kind,
        Match,
        Query,
        Reference,
    },
    persistence::db::{
        self,
        works::{
            self,
            MatchedBy,
        },
    },
};

use crate::{
    events::names,
    state::AppState,
};

/// Uncertain matches by source, for the page to offer until the user answers.
static SUGGESTIONS: Mutex<Option<HashMap<String, Match>>> = Mutex::new(None);

fn suggestions<T>(f: impl FnOnce(&mut HashMap<String, Match>) -> T) -> T {
    f(SUGGESTIONS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_default())
}

#[derive(Serialize, Clone)]
pub struct MediaMatchDto {
    pub fingerprint: String,
    #[serde(flatten)]
    pub found: Match,
    /// `name`, `content` or `user` for a link; `suggested` for an uncertain match.
    pub matched_by: &'static str,
}

/// The source's match, or a suggestion when it has none.
#[tauri::command]
pub fn get_media_match(fingerprint: String) -> Option<MediaMatchDto> {
    if let Some((found, by)) =
        db::with(|conn| works::source_match(conn, &fingerprint)).ok().flatten()
    {
        return Some(MediaMatchDto { fingerprint, found, matched_by: by.as_str() });
    }
    let found = suggestions(|s| s.get(&fingerprint).cloned())?;
    Some(MediaMatchDto { fingerprint, found, matched_by: "suggested" })
}

#[derive(Serialize)]
pub struct MediaSearch {
    pub kind: Kind,
    pub results: Vec<Candidate>,
    /// The season and episode of a pasted episode link.
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// Searches TMDB by title, or finds the entry a pasted TMDB link or ID names.
#[tauri::command]
pub async fn search_media(app: AppHandle, text: String, kind: Kind) -> Result<MediaSearch, String> {
    let tmdb = tmdb_client(&app)?;
    let reference = Reference::parse(&text, kind);
    let mut search = MediaSearch {
        kind: reference.as_ref().map_or(kind, |r| r.kind),
        results: Vec::new(),
        season: reference.as_ref().and_then(|r| r.season),
        episode: reference.as_ref().and_then(|r| r.episode),
    };
    if let Some(r) = &reference {
        search.results.extend(tmdb.details(r.kind, r.tmdb_id).await.map_err(|e| e.to_string())?);
    }
    // A bare number may also be a title, like 86.
    if !text.contains("themoviedb.org") {
        for candidate in tmdb.search(kind, &text, None).await.map_err(|e| e.to_string())? {
            if search.results.iter().all(|c| c.id != candidate.id) {
                search.results.push(candidate);
            }
        }
    }
    Ok(search)
}

/// Links the source to what the user named, and points its title's other files there too.
#[tauri::command]
pub async fn set_media_match(
    app: AppHandle,
    fingerprint: String,
    reference: Reference,
) -> Result<MediaMatchDto, String> {
    let tmdb = tmdb_client(&app)?;
    let found = catalog::lookup(&tmdb, &reference)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("TMDB has no entry with that ID")?;
    db::with(|conn| works::link(conn, &fingerprint, &found, MatchedBy::User))?;
    suggestions(|s| s.remove(&fingerprint));
    if let Some(query) = query_for(&fingerprint)? {
        catalog::remember(&query, &found).map_err(|e| e.to_string())?;
        relink(&tmdb, &query, &fingerprint).await;
    }
    Ok(MediaMatchDto { fingerprint, found, matched_by: MatchedBy::User.as_str() })
}

/// Records that the source isn't a show or film, and neither are its title's other files.
#[tauri::command]
pub async fn clear_media_match(app: AppHandle, fingerprint: String) -> Result<(), String> {
    db::with(|conn| works::unlink(conn, &fingerprint, true))?;
    suggestions(|s| s.remove(&fingerprint));
    if let Some(query) = query_for(&fingerprint)? {
        catalog::forget(&query).map_err(|e| e.to_string())?;
        if let Ok(tmdb) = tmdb_client(&app) {
            relink(&tmdb, &query, &fingerprint).await;
        }
    }
    Ok(())
}

fn tmdb_client(app: &AppHandle) -> Result<Tmdb, String> {
    let user_key = app.state::<Mutex<AppState>>().lock().unwrap().settings.tmdb_api_key.clone();
    tmdb::api_key(Some(&user_key))
        .map(Tmdb::new)
        .ok_or_else(|| "This build has no TMDB key; add your own in settings".to_string())
}

/// What the source's name says it is, from the path it was last opened from.
fn query_for(fingerprint: &str) -> Result<Option<Query>, String> {
    let path = db::with(|conn| works::opened_path(conn, fingerprint))?;
    Ok(path.as_deref().and_then(Query::from_name))
}

/// Matches again the sources matched by a name with the corrected query's title.
async fn relink(tmdb: &Tmdb, corrected: &Query, except: &str) {
    let named = match db::with(|conn| works::matched_by_name(conn)) {
        Ok(named) => named,
        Err(e) => return eprintln!("[catalog] Couldn't list matched sources: {e}"),
    };
    let key = corrected.key();
    for (fingerprint, path) in named {
        let Some(query) =
            Query::from_name(&path).filter(|q| fingerprint != except && q.key() == key)
        else {
            continue;
        };
        let result = match catalog::resolve(tmdb, &query).await {
            Ok(Some(resolved)) if resolved.confident => {
                db::with(|conn| works::link(conn, &fingerprint, &resolved.found, MatchedBy::Name))
            }
            Ok(_) => db::with(|conn| works::unlink(conn, &fingerprint, false)),
            Err(e) => Err(e.to_string()),
        };
        if let Err(e) = result {
            eprintln!("[catalog] Matching {path} again failed: {e}");
        }
    }
}

/// Matches the source in the background, and emits `media-matched` when it links one or has
/// a suggestion. `name` is what was opened: the video, or the subtitle file.
pub fn spawn(app: AppHandle, fingerprint: String, name: String, lines: Vec<String>) {
    tauri::async_runtime::spawn(async move {
        let found = match find(&app, &fingerprint, &name, &lines).await {
            Ok(found) => found,
            Err(e) => {
                eprintln!("[catalog] Matching {name} failed: {e}");
                return;
            }
        };
        if let Some((found, matched_by)) = found {
            let _ =
                app.emit(names::MEDIA_MATCHED, MediaMatchDto { fingerprint, found, matched_by });
        }
    });
}

/// A new link for the source, or a suggestion.
async fn find(
    app: &AppHandle,
    fingerprint: &str,
    name: &str,
    lines: &[String],
) -> Result<Option<(Match, &'static str)>, String> {
    let decided = db::with(|conn| {
        Ok(works::user_decided(conn, fingerprint)?
            || works::source_match(conn, fingerprint)?.is_some())
    })?;
    if decided {
        return Ok(None);
    }
    if let Some(signature) = Signature::of(lines.iter().map(String::as_str)) {
        let same = db::with(|conn| {
            works::set_signature(conn, fingerprint, &signature)?;
            works::same_episode(conn, fingerprint, &signature)
        })?;
        if let Some(found) = same {
            db::with(|conn| works::link(conn, fingerprint, &found, MatchedBy::Content))?;
            return Ok(Some((found, MatchedBy::Content.as_str())));
        }
    }
    let Some(query) = Query::from_name(name) else { return Ok(None) };
    let Ok(tmdb) = tmdb_client(app) else { return Ok(None) };
    match catalog::resolve(&tmdb, &query).await.map_err(|e| e.to_string())? {
        Some(resolved) if resolved.confident => {
            db::with(|conn| works::link(conn, fingerprint, &resolved.found, MatchedBy::Name))?;
            Ok(Some((resolved.found, MatchedBy::Name.as_str())))
        }
        Some(resolved) => {
            suggestions(|s| s.insert(fingerprint.to_string(), resolved.found.clone()));
            Ok(Some((resolved.found, "suggested")))
        }
        None => Ok(None),
    }
}
