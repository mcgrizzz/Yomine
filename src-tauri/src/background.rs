//! Ambient background loops; each pushes its event only on change.

use std::{
    collections::HashSet,
    sync::{
        atomic::Ordering,
        Mutex,
    },
    time::Duration,
};

use tauri::{
    AppHandle,
    Emitter,
    Manager,
};
use yomine::{
    anki,
    tools::knowledge_summary::compute_knowledge_summary,
    yomitan,
};

use crate::{
    commands::file::load_asbplayer_into_state,
    dto::{
        KnowledgeSummaryDto,
        YomitanStatusDto,
    },
    events::{
        names,
        AnkiStatus,
        ErrorPayload,
    },
    player_task::PlayerHandle,
    state::AppState,
};

/// Same 5s cadence egui throttled the Anki probe to.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// The knowledge check when idle is just a lock + atomic read, so it can spin
/// fast — the summary then lands within ~1s of the tools (or a dirty flag).
const KNOWLEDGE_POLL_INTERVAL: Duration = Duration::from_secs(1);

// Polling because asbplayer has no push for media changes; the cadence setting
// is re-read every iteration, clamped to ≥1s.

/// Spawn the background poll loops. Call once at app setup.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(poll_anki(app.clone()));
    tauri::async_runtime::spawn(poll_yomitan(app.clone()));
    tauri::async_runtime::spawn(poll_knowledge(app.clone()));
    tauri::async_runtime::spawn(poll_asbplayer_follow(app));
}

/// Anki connectivity probe: emit `anki-status` only on change (initial poll
/// fires at t=0).
async fn poll_anki(app: AppHandle) {
    let mut last_connected: Option<bool> = None;
    let mut tick = tokio::time::interval(POLL_INTERVAL);

    loop {
        tick.tick().await;

        let connected = anki::api::get_version().await.is_ok();
        if last_connected != Some(connected) {
            let _ = app.emit(names::ANKI_STATUS, AnkiStatus { connected, fetching: false });
            last_connected = Some(connected);
        }
    }
}

/// yomitan-api reachability probe (drives the TopBar dot and the mine-button
/// gate even when nothing else pulls it): emit `yomitan-status` on change.
async fn poll_yomitan(app: AppHandle) {
    let mut last: Option<YomitanStatusDto> = None;
    let mut tick = tokio::time::interval(POLL_INTERVAL);

    loop {
        tick.tick().await;

        let url = {
            let state = app.state::<Mutex<AppState>>();
            let guard = state.lock().unwrap();
            guard.settings.yomitan_url.clone()
        };
        let status = match yomitan::get_version(&url).await {
            Ok(version) => YomitanStatusDto { reachable: true, version: Some(version) },
            Err(_) => YomitanStatusDto { reachable: false, version: None },
        };
        if last.as_ref() != Some(&status) {
            let _ = app.emit(names::YOMITAN_STATUS, status.clone());
            last = Some(status);
        }
    }
}

/// Its own loop so it never queues behind the Anki probe's timeout — it only
/// needs the offline vocab cache.
async fn poll_knowledge(app: AppHandle) {
    let mut tick = tokio::time::interval(KNOWLEDGE_POLL_INTERVAL);

    loop {
        tick.tick().await;

        let pending = {
            let state = app.state::<Mutex<AppState>>();
            let guard = state.lock().unwrap();
            match &guard.language_tools {
                Some(tools) if guard.knowledge_dirty.load(Ordering::Relaxed) => Some((
                    tools.frequency_manager.clone(),
                    tools.known_interval,
                    guard.knowledge_dirty.clone(),
                )),
                _ => None,
            }
        };
        if let Some((frequency_manager, known_interval, dirty)) = pending {
            // Reads the offline Anki vocab cache; only meaningful once it exists.
            if anki::has_cached_vocab() {
                if let Ok(summary) = tauri::async_runtime::spawn_blocking(move || {
                    compute_knowledge_summary(frequency_manager, known_interval)
                })
                .await
                {
                    // Cached for the one-shot pull; pushed to any live webview.
                    let dto = KnowledgeSummaryDto::from_summary(summary);
                    {
                        let state = app.state::<Mutex<AppState>>();
                        state.lock().unwrap().knowledge_summary = Some(dto.clone());
                    }
                    let _ = app.emit(names::KNOWLEDGE_SUMMARY, dto);
                    dirty.store(false, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Follow mode, armed only while the current content came from asbplayer.
/// New-media rule: arming seeds the seen-set with everything currently bound,
/// so only genuinely-new ids (with subtitles) trigger — tab switching doesn't.
/// Active-tab rule: switch when the active tab's subtitled video isn't what's
/// loaded (a loaded-∈-actives no-op keeps two active windows from flapping).
async fn poll_asbplayer_follow(app: AppHandle) {
    // `None` = disarmed; `Some(ids)` = armed with the media ids already seen.
    let mut seen: Option<HashSet<String>> = None;

    loop {
        let (armed, follow_new, follow_active, poll_secs, current_media_id) = {
            let state = app.state::<Mutex<AppState>>();
            let guard = state.lock().unwrap();
            let follow_new = guard.settings.asbplayer_follow_new_media;
            let follow_active = guard.settings.asbplayer_follow_active_tab;
            let armed = (follow_new || follow_active)
                && guard.language_tools.is_some()
                && guard.file.asbplayer_media_id.is_some();
            (
                armed,
                follow_new,
                follow_active,
                guard.settings.asbplayer_poll_secs.max(1),
                guard.file.asbplayer_media_id.clone(),
            )
        };
        tokio::time::sleep(Duration::from_secs(poll_secs as u64)).await;
        if !armed {
            seen = None;
            continue;
        }

        let player = app.state::<PlayerHandle>();
        // Not connected / extension too old → just keep waiting.
        let Ok(media) = player.get_bound_media().await else { continue };

        let Some(seen_ids) = &mut seen else {
            // Just armed: everything currently bound is old news.
            seen = Some(media.iter().map(|m| m.id.clone()).collect());
            continue;
        };

        // 1) New media (once its subtitles are loaded — they often land a poll
        //    or two after the video appears). Prefer the active tab.
        let mut target = None;
        if follow_new {
            let mut fresh: Vec<_> = media
                .iter()
                .filter(|m| !seen_ids.contains(&m.id) && !m.loaded_subtitles.is_empty())
                .collect();
            fresh.sort_by_key(|m| !m.active);
            target = fresh.first().copied();
        }

        // 2) Active-tab follow: the loaded video is no longer (one of) the
        //    active-with-subtitles tabs → switch to the first one that is.
        if target.is_none() && follow_active {
            let actives: Vec<_> =
                media.iter().filter(|m| m.active && !m.loaded_subtitles.is_empty()).collect();
            let current_is_active =
                current_media_id.as_ref().is_some_and(|id| actives.iter().any(|m| &m.id == id));
            if !current_is_active {
                target = actives.first().copied();
            }
        }

        let Some(next) = target else { continue };
        seen_ids.insert(next.id.clone());
        let title = next.title.clone().unwrap_or_else(|| "asbplayer video".to_string());
        let file_name = next.loaded_subtitles.first().map(|t| t.file_name.clone());
        match load_asbplayer_into_state(
            &app,
            &player,
            next.id.clone(),
            None,
            title,
            file_name,
            None,
        )
        .await
        {
            Ok(payload) => {
                let _ = app.emit(names::ASBPLAYER_MEDIA_LOADED, payload);
            }
            Err(e) => {
                let _ = app.emit(
                    names::ERROR,
                    ErrorPayload {
                        title: "asbplayer follow".into(),
                        message: "Failed to load the new video".into(),
                        detail: Some(e),
                    },
                );
            }
        }
    }
}
