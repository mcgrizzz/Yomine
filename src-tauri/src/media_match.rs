//! Matching a loaded file to its show or film after the load, without holding it up, and
//! the user's corrections.

use std::{
    collections::{
        HashMap,
        HashSet,
    },
    path::{
        Path,
        PathBuf,
    },
    sync::Mutex,
    time::SystemTime,
};

use serde::Serialize;
use tauri::{
    ipc::Channel,
    AppHandle,
    Emitter,
    Manager,
};
use yomine::{
    catalog::{
        self,
        signature::Signature,
        store,
        tmdb::{
            self,
            Candidate,
            Episode,
            Tmdb,
        },
        Kind,
        Match,
        Query,
        Reference,
    },
    media::{
        ffmpeg,
        subtitles,
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
        let found = catalog::displayed(found);
        return Some(MediaMatchDto { fingerprint, found, matched_by: by.as_str() });
    }
    let found = catalog::displayed(suggestions(|s| s.get(&fingerprint).cloned())?);
    Some(MediaMatchDto { fingerprint, found, matched_by: "suggested" })
}

/// The linked matches of these sources, by fingerprint; unmatched sources are left out.
#[tauri::command]
pub fn get_media_matches(fingerprints: Vec<String>) -> Result<HashMap<String, Match>, String> {
    db::with(|conn| {
        let mut found = HashMap::new();
        for fingerprint in fingerprints {
            if let Some((m, _)) = works::source_match(conn, &fingerprint)? {
                found.insert(fingerprint, catalog::displayed(m));
            }
        }
        Ok(found)
    })
}

/// The season's aired episodes, numbered as its matches are shown; `None` without a TMDB key.
#[tauri::command]
pub async fn get_season_episodes(
    app: AppHandle,
    show: i64,
    season: u32,
) -> Result<Option<Vec<Episode>>, String> {
    let Ok(tmdb) = tmdb_client(&app) else { return Ok(None) };
    catalog::aired_episodes(&tmdb, show, season).await.map(Some).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct MinedEpisode {
    pub show: i64,
    pub season: Option<u32>,
    pub episode: u32,
}

/// The episodes the active collection has notes from, numbered as their matches are shown.
#[tauri::command]
pub async fn get_mined_episodes() -> Result<Vec<MinedEpisode>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mined =
            db::with(|conn| works::mined_episodes(conn, &db::anki::active_collection(conn)?))?;
        Ok(mined
            .into_iter()
            .map(|(show, season, episode)| match season {
                Some(season) => {
                    let (season, episode) = catalog::displayed_episode(show, season, episode);
                    MinedEpisode { show, season: Some(season), episode }
                }
                None => MinedEpisode { show, season, episode },
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
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
        // A link has TMDB's numbering; the panel shows the layout's.
        if let (Kind::Tv, Some(season), Some(episode)) = (r.kind, r.season, r.episode) {
            let layout = catalog::layout(&tmdb, r.tmdb_id).await.map_err(|e| e.to_string())?;
            if let Some((season, episode)) = layout.and_then(|l| l.from_tmdb(season, episode)) {
                search.season = Some(season);
                search.episode = Some(episode);
            }
        }
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
    let found = catalog::displayed(found);
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

#[derive(Serialize, Clone)]
pub struct FoundEpisode {
    pub path: String,
    #[serde(flatten)]
    pub found: Match,
    /// Whether it has Japanese or untagged subtitles to mine; `None` while that's checked.
    pub subtitles: Option<bool>,
}

/// The episodes in the folders of these opened files, as `(path, show)`, of the shows each
/// folder's opened files are matched to: its videos, and subtitle files no video pairs with.
/// What a file is stays known until it changes, and `rescan` looks at every file again.
/// Each is sent as soon as it's matched, then again once its subtitles are checked. Nothing
/// found is added to the history.
#[tauri::command]
pub async fn find_episodes(
    app: AppHandle,
    opened: Vec<(String, i64)>,
    rescan: bool,
    found: Channel<FoundEpisode>,
) -> Result<(), String> {
    let send = move |episode: FoundEpisode| {
        if let Err(e) = found.send(episode) {
            eprintln!("[catalog] {e}");
        }
    };
    let Ok(tmdb) = tmdb_client(&app) else { return Ok(()) };
    let mut shows: HashMap<PathBuf, HashSet<i64>> = HashMap::new();
    for (path, show) in &opened {
        if let Some(dir) = Path::new(path).parent() {
            shows.entry(dir.to_path_buf()).or_default().insert(*show);
        }
    }
    let dirs: Vec<PathBuf> = shows.keys().cloned().collect();
    let listed = tauri::async_runtime::spawn_blocking(move || {
        dirs.into_iter().map(|dir| (subtitles::episodes_in(&dir), dir)).collect::<Vec<_>>()
    })
    .await
    .map_err(|e| e.to_string())?;
    let opened: HashSet<String> = opened.into_iter().map(|(path, _)| path).collect();
    let mut matched = Vec::new();
    let mut offline = false;
    for (files, dir) in listed {
        for (file, meta) in files {
            let path = file.display().to_string();
            if opened.contains(&path) {
                continue;
            }
            let stamp = store::FileStamp {
                size: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64),
            };
            let cached = if rescan {
                None
            } else {
                store::found_file(&path, stamp).unwrap_or_else(|e| {
                    eprintln!("[catalog] {e}");
                    None
                })
            };
            let (matched_to, checked) = match cached {
                Some(cached) => {
                    let checked = Checked::from(&cached);
                    (cached.found, checked)
                }
                None if offline => continue,
                None => {
                    let Some(query) = Query::from_name(&path) else { continue };
                    match catalog::resolve(&tmdb, &query).await {
                        Ok(resolved) => {
                            let found = resolved.map(|r| r.found);
                            if let Err(e) =
                                store::set_found_file(&path, stamp, &query.key(), found.as_ref())
                            {
                                eprintln!("[catalog] {e}");
                            }
                            (found, Checked::Unknown)
                        }
                        // Offline or refused: the rest would fail the same way.
                        Err(e) => {
                            eprintln!("[catalog] Matching found files stopped at {path}: {e}");
                            offline = true;
                            continue;
                        }
                    }
                }
            };
            let Some(matched_to) =
                matched_to.filter(|f| f.episode.is_some() && shows[&dir].contains(&f.work.tmdb_id))
            else {
                continue;
            };
            let episode = FoundEpisode {
                path,
                found: catalog::displayed(matched_to),
                subtitles: (checked == Checked::Subtitles).then_some(true),
            };
            send(episode.clone());
            if checked != Checked::Subtitles {
                matched.push((file, episode, checked));
            }
        }
    }
    let configured = app.state::<Mutex<AppState>>().lock().unwrap().settings.ffmpeg_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let configured = Some(configured.trim()).filter(|p| !p.is_empty()).map(Path::new);
        let probing = matched.iter().any(|(.., checked)| *checked == Checked::Unknown);
        let ffmpeg = probing.then(|| ffmpeg::find(configured)).flatten();
        for (file, episode, checked) in matched {
            let subtitles = has_minable(&file, &episode.path, checked, ffmpeg.as_ref());
            send(FoundEpisode { subtitles: Some(subtitles), ..episode });
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// What's known of a found file's subtitles.
#[derive(Clone, Copy, PartialEq)]
enum Checked {
    Unknown,
    Subtitles,
    /// No usable track inside, though a subtitle file may be added beside it.
    NoTrack,
}

impl From<&store::FoundFile> for Checked {
    fn from(file: &store::FoundFile) -> Self {
        match (file.subtitles, file.probed) {
            (true, _) => Checked::Subtitles,
            (false, true) => Checked::NoTrack,
            (false, false) => Checked::Unknown,
        }
    }
}

/// Probes a video's tracks only once while it's unchanged, since that reads it from what may
/// be a network share. A subtitle file beside it is looked for every time.
fn has_minable(file: &Path, path: &str, checked: Checked, ffmpeg: Option<&ffmpeg::Ffmpeg>) -> bool {
    let has = !subtitles::is_video(file)
        || subtitles::has_minable_beside(file)
        || ffmpeg
            .filter(|_| checked == Checked::Unknown)
            .is_some_and(|ffmpeg| has_minable_track(file, path, ffmpeg));
    if has {
        if let Err(e) = store::set_found_subtitles(path) {
            eprintln!("[catalog] {e}");
        }
    }
    has
}

fn has_minable_track(file: &Path, path: &str, ffmpeg: &ffmpeg::Ffmpeg) -> bool {
    match subtitles::has_minable_track(file, ffmpeg) {
        Ok(true) => true,
        Ok(false) => {
            if let Err(e) = store::set_found_probed(path) {
                eprintln!("[catalog] {e}");
            }
            false
        }
        Err(e) => {
            eprintln!("[catalog] Couldn't probe {path}: {e}");
            false
        }
    }
}

/// Whether this build embeds a TMDB key, so matching works without the user's own.
#[tauri::command]
pub fn has_builtin_tmdb_key() -> bool {
    tmdb::api_key(None).is_some()
}

fn tmdb_client(app: &AppHandle) -> Result<Tmdb, String> {
    let user_key = app.state::<Mutex<AppState>>().lock().unwrap().settings.tmdb_api_key.clone();
    tmdb::api_key(Some(&user_key)).map(Tmdb::new).ok_or_else(|| {
        "This build has no TMDB key; add your own in Settings → Show Matching".to_string()
    })
}

/// What the source's name says it is, from the path it was last opened from.
fn query_for(fingerprint: &str) -> Result<Option<Query>, String> {
    let path = db::with(|conn| works::opened_path(conn, fingerprint))?;
    Ok(path.as_deref().and_then(Query::from_name))
}

/// Matches again the sources matched by a name with the corrected query's title.
async fn relink(tmdb: &Tmdb, corrected: &Query, except: &str) {
    let named = match db::with(|conn| works::matched_by(conn, Some(MatchedBy::Name))) {
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

/// Matches by name the subtitle sources loaded before matching existed, or while TMDB
/// couldn't be reached. Uncertain matches are left for the user to make on a load. Also
/// finds the layout of each matched show without one cached, so its episodes show in it.
pub fn spawn_past(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(tmdb) = tmdb_client(&app) else { return };
        let unmatched = match db::with(|conn| works::matched_by(conn, None)) {
            Ok(unmatched) => unmatched,
            Err(e) => return eprintln!("[catalog] Couldn't list unmatched sources: {e}"),
        };
        let mut linked = false;
        for show in db::with(|conn| works::shows(conn)).unwrap_or_default() {
            if matches!(store::layout(show), Ok(Some(_))) {
                continue;
            }
            match catalog::layout(&tmdb, show).await {
                Ok(layout) => linked |= layout.is_some(),
                Err(e) => {
                    eprintln!("[catalog] Finding the seasons of show {show} failed: {e}");
                    break;
                }
            }
        }
        for (fingerprint, path) in unmatched {
            let Some(query) = Query::from_name(&path) else { continue };
            match catalog::resolve(&tmdb, &query).await {
                Ok(Some(resolved)) if resolved.confident => {
                    let found = &resolved.found;
                    match db::with(|conn| works::link(conn, &fingerprint, found, MatchedBy::Name)) {
                        Ok(()) => linked = true,
                        Err(e) => eprintln!("[catalog] Linking {path} failed: {e}"),
                    }
                }
                Ok(_) => {}
                // Offline or refused: the rest would fail the same way.
                Err(e) => {
                    eprintln!("[catalog] Matching past files stopped at {path}: {e}");
                    break;
                }
            }
        }
        if linked {
            let _ = app.emit(names::PAST_MEDIA_MATCHED, ());
        }
    });
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
            let found = catalog::displayed(found);
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
