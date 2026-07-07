//! Ambient background polling (T020): the Anki connectivity probe and the
//! knowledge-summary recompute, replacing egui's per-frame `update_anki_status` /
//! `maybe_compute_knowledge_summary`. Player connectivity has its own task
//! (`player_task`). Pushes `anki-status` / `knowledge-summary` events (R5).

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
};

use crate::{
    commands::file::load_asbplayer_into_state,
    dto::KnowledgeSummaryDto,
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

// Follow mode's poll cadence is the persisted `asbplayer_poll_secs` setting
// (asbplayer has no push notification for media changes); it is re-read every
// iteration, clamped to ≥1s. Idle cost per tick: one AppState lock.

/// Spawn the background poll loops. Call once at app setup.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(poll_anki(app.clone()));
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

/// Knowledge-summary recompute, in its own loop so it never queues behind the
/// Anki probe — it only needs the offline vocab cache, but sharing the probe's
/// tick meant the first compute waited out the AnkiConnect attempt (and its
/// timeout when Anki is closed), so the widget appeared late (maintainer
/// report, 2026-07-06). Recomputes when an input changed and a cache exists.
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
                    // Cache for the `get_knowledge_summary` pull, then push the same
                    // DTO to any live webview (the engine tuples are reshaped to named
                    // fields once, here, so the wire format matches the TS interface).
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

/// asbplayer follow mode (issue #105): while ARMED — a follow setting is on AND
/// the current content was loaded from asbplayer — watch `get-bound-media` and
/// automatically load:
/// - **new media** (`asbplayer_follow_new_media`): a media id we haven't seen
///   since arming, once it has subtitles (the next episode, autoplay or manual).
///   Arming seeds the seen-set with everything currently bound, so switching
///   between already-open tabs never triggers this.
/// - **the active tab** (`asbplayer_follow_active_tab`): when the active tab's
///   video (with subtitles) isn't what's loaded, switch to it — this one DOES
///   follow tab switches.
/// Disarms (and re-seeds on the next arm) when both settings are off or a
/// regular file is opened. Successful loads push the new `FileLoadResult` via
/// `asbplayer-media-loaded`; failures surface on the `error` event.
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
        match load_asbplayer_into_state(&app, &player, next.id.clone(), None, title, None).await {
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
