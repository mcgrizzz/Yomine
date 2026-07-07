//! Ambient background polling (T020): the Anki connectivity probe and the
//! knowledge-summary recompute, replacing egui's per-frame `update_anki_status` /
//! `maybe_compute_knowledge_summary`. Player connectivity has its own task
//! (`player_task`). Pushes `anki-status` / `knowledge-summary` events (R5).

use std::{
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
    dto::KnowledgeSummaryDto,
    events::{
        names,
        AnkiStatus,
    },
    state::AppState,
};

/// Same 5s cadence egui throttled the Anki probe to.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// The knowledge check when idle is just a lock + atomic read, so it can spin
/// fast — the summary then lands within ~1s of the tools (or a dirty flag).
const KNOWLEDGE_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Spawn the background poll loops. Call once at app setup.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(poll_anki(app.clone()));
    tauri::async_runtime::spawn(poll_knowledge(app));
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
