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

/// yomitan-api reachability probe: emit `yomitan-status` on change.
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

/// asbplayer follow mode + the `asbplayer-context` awareness event.
async fn poll_asbplayer_follow(app: AppHandle) {
    // `None` = disarmed; `Some(ids)` = armed with the media ids already seen.
    let mut seen: Option<HashSet<String>> = None;
    let mut prev_actives: Option<HashSet<String>> = None;
    let mut last_ctx: Option<crate::events::AsbplayerContext> = None;

    loop {
        let (armed, follow_new, follow_active, poll_secs, current_media_id) = {
            let state = app.state::<Mutex<AppState>>();
            let guard = state.lock().unwrap();
            let follow_new = guard.settings.asbplayer_follow_new_media;
            let follow_active = guard.settings.asbplayer_follow_active_tab;
            let armed = (follow_new || follow_active) && guard.language_tools.is_some();
            (
                armed,
                follow_new,
                follow_active,
                guard.settings.asbplayer_poll_secs.max(1),
                guard.file.asbplayer_media_id.clone(),
            )
        };
        tokio::time::sleep(Duration::from_secs(poll_secs as u64)).await;

        let player = app.state::<PlayerHandle>();
        // Only ask asbplayer when someone consumes the answer (follow or an
        // asbplayer session) and a client is actually connected.
        let relevant = follow_new || follow_active || current_media_id.is_some();
        let clients = player.status().await.map(|s| s.ws_clients).unwrap_or(0);
        if !relevant || clients == 0 {
            seen = None;
            prev_actives = None;
            if last_ctx.take().is_some() {
                let _ =
                    app.emit(names::ASBPLAYER_CONTEXT, crate::events::AsbplayerContext::default());
            }
            continue;
        }
        // Extension too old for get-bound-media → clear stale context, keep waiting.
        let Ok(media) = player.get_bound_media().await else {
            seen = None;
            prev_actives = None;
            if last_ctx.take().is_some() {
                let _ =
                    app.emit(names::ASBPLAYER_CONTEXT, crate::events::AsbplayerContext::default());
            }
            continue;
        };

        let actives_now: HashSet<String> = media
            .iter()
            .filter(|m| m.active && !m.loaded_subtitles.is_empty())
            .map(|m| m.id.clone())
            .collect();

        let active = media.iter().find(|m| m.active);
        let ctx = crate::events::AsbplayerContext {
            has_active_tab: active.is_some(),
            active_title: active.and_then(|m| m.title.clone()),
            active_has_subtitles: active.is_some_and(|m| !m.loaded_subtitles.is_empty()),
            loaded_is_active: current_media_id.as_ref().is_some_and(|id| actives_now.contains(id)),
            loaded_from_asbplayer: current_media_id.is_some(),
        };
        if last_ctx.as_ref() != Some(&ctx) {
            let _ = app.emit(names::ASBPLAYER_CONTEXT, ctx.clone());
            last_ctx = Some(ctx);
        }

        if !armed {
            seen = None;
            prev_actives = None;
            continue;
        }

        let actives_changed = prev_actives.as_ref().is_some_and(|p| p != &actives_now);
        let just_armed = seen.is_none();
        prev_actives = Some(actives_now.clone());
        if just_armed {
            // Just armed: everything currently bound is old news.
            seen = Some(media.iter().map(|m| m.id.clone()).collect());
            continue;
        }
        let seen_ids = seen.as_mut().expect("seeded above");

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
            let current_is_active =
                current_media_id.as_ref().is_some_and(|id| actives_now.contains(id));
            let should_switch = if current_media_id.is_some() {
                !current_is_active
            } else {
                // File-sourced session: only a fresh activation switches away.
                actives_changed
            };
            if should_switch {
                target = media.iter().find(|m| m.active && !m.loaded_subtitles.is_empty());
            }
        }

        let Some(next) = target else { continue };
        seen_ids.insert(next.id.clone());
        let title = next.title.clone().unwrap_or_else(|| "asbplayer video".to_string());
        let file_name = next.loaded_subtitles.first().map(|t| t.file_name.clone());
        // Same subtitle file already loaded (e.g. the same episode in another
        // tab) → adopt the new media id instead of re-downloading.
        {
            let state = app.state::<Mutex<AppState>>();
            let mut guard = state.lock().unwrap();
            if file_name.is_some() && guard.file.asbplayer_subtitle_file == file_name {
                guard.file.asbplayer_media_id = Some(next.id.clone());
                continue;
            }
        }
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
