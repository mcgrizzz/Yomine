//! Matching a loaded file to its show or film after the load, without holding it up.

use std::sync::Mutex;

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
            Tmdb,
        },
        Match,
        Query,
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

#[derive(Serialize, Clone)]
pub struct MediaMatchDto {
    pub fingerprint: String,
    #[serde(flatten)]
    pub found: Match,
    pub matched_by: &'static str,
}

/// The source's match, if it has one.
#[tauri::command]
pub fn get_media_match(fingerprint: String) -> Option<MediaMatchDto> {
    let found = db::with(|conn| works::source_match(conn, &fingerprint)).ok().flatten()?;
    Some(MediaMatchDto { fingerprint, found: found.0, matched_by: found.1.as_str() })
}

/// Matches the source in the background, and emits `media-matched` when it links one. `name`
/// is what was opened: the video, or the subtitle file.
pub fn spawn(app: AppHandle, fingerprint: String, name: String, lines: Vec<String>) {
    tauri::async_runtime::spawn(async move {
        let found = match find(&app, &fingerprint, &name, &lines).await {
            Ok(found) => found,
            Err(e) => {
                eprintln!("[catalog] Matching {name} failed: {e}");
                return;
            }
        };
        if let Some((found, by)) = found {
            let _ = app.emit(
                names::MEDIA_MATCHED,
                MediaMatchDto { fingerprint, found, matched_by: by.as_str() },
            );
        }
    });
}

/// A new link for the source, if it gets one.
async fn find(
    app: &AppHandle,
    fingerprint: &str,
    name: &str,
    lines: &[String],
) -> Result<Option<(Match, MatchedBy)>, String> {
    if db::with(|conn| works::source_match(conn, fingerprint))?.is_some() {
        return Ok(None);
    }
    if let Some(signature) = Signature::of(lines.iter().map(String::as_str)) {
        let same = db::with(|conn| {
            works::set_signature(conn, fingerprint, &signature)?;
            works::same_episode(conn, fingerprint, &signature)
        })?;
        if let Some(found) = same {
            return link(fingerprint, found, MatchedBy::Content);
        }
    }
    let Some(query) = Query::from_name(name) else { return Ok(None) };
    let user_key = app.state::<Mutex<AppState>>().lock().unwrap().settings.tmdb_api_key.clone();
    let Some(key) = tmdb::api_key(Some(&user_key)) else { return Ok(None) };
    match catalog::resolve(&Tmdb::new(key), &query).await.map_err(|e| e.to_string())? {
        Some(resolved) if resolved.confident => link(fingerprint, resolved.found, MatchedBy::Name),
        _ => Ok(None),
    }
}

fn link(
    fingerprint: &str,
    found: Match,
    by: MatchedBy,
) -> Result<Option<(Match, MatchedBy)>, String> {
    db::with(|conn| works::link(conn, fingerprint, &found, by))?;
    Ok(Some((found, by)))
}
