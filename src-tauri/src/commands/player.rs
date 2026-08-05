//! Player commands (contracts/commands.md "Player"): thin wrappers over the
//! `PlayerHandle` channel — the player is owned by its task, never this lock.

use std::sync::Mutex;

use base64::Engine;
use tauri::{
    AppHandle,
    Emitter,
    State,
};
use yomine::{
    core::models::SourceFileType,
    persistence,
};

use crate::{
    events::{
        names,
        PlayerStatus,
    },
    player_task::PlayerHandle,
    state::AppState,
};

/// Seek the active player to `seconds` (prefers MPV, else the WebSocket client).
/// Errors if no player is connected (the handle relays the player's own error).
#[tauri::command]
pub async fn seek_timestamp(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    seconds: f32,
    label: String,
) -> Result<(), String> {
    let media_id = { state.lock().unwrap().file.asbplayer_media_id.clone() };
    player.seek(seconds, label, media_id).await
}

/// Current player connectivity/mode. Also pushed via the `player-status` event.
#[tauri::command]
pub async fn get_player_status(player: State<'_, PlayerHandle>) -> Result<PlayerStatus, String> {
    player.status().await
}

/// Persist the port and move a running server to it; a not-yet-started server
/// picks the port up on its next tick.
#[tauri::command]
pub fn set_websocket_port(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    port: u16,
) -> Result<(), String> {
    let settings_to_save = {
        let mut guard = state.lock().unwrap();
        guard.settings.websocket_settings.port = port;
        guard.settings.clone()
    };
    persistence::save_json(&settings_to_save, "settings.json").map_err(|e| e.to_string())?;
    player.set_port(port);
    Ok(())
}

/// The media asbplayer is currently tracking (`get-bound-media`, issue #105) —
/// drives the "Load from asbplayer" picker. Errors when asbplayer isn't
/// connected or the extension predates the command (v1.20+).
#[tauri::command]
pub async fn get_asbplayer_media(
    player: State<'_, PlayerHandle>,
) -> Result<Vec<crate::dto::BoundMediaDto>, String> {
    Ok(player.get_bound_media().await?.into_iter().map(Into::into).collect())
}

/// Re-point mining/seeking at another asbplayer video, keeping the loaded
/// subtitles. Emits a fresh `asbplayer-context` so the UI updates immediately.
#[tauri::command]
pub async fn set_asbplayer_target(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    media_id: String,
) -> Result<(), String> {
    {
        let mut guard = state.lock().unwrap();
        if guard.file.source_file.is_none() {
            return Err("Load a file before choosing a target video".to_string());
        }
        guard.file.asbplayer_media_id = Some(media_id.clone());
    }
    if let Ok(media) = player.get_bound_media().await {
        let ctx = crate::background::asbplayer_context(&media, Some(&media_id));
        let _ = app.emit(names::ASBPLAYER_CONTEXT, ctx);
    }
    Ok(())
}

/// Push the current session's subtitle file to asbplayer (`load-subtitles`).
/// asbplayer opens its video-select overlay in the active tab.
#[tauri::command]
pub async fn send_subtitles_to_asbplayer(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
) -> Result<(), String> {
    let path = {
        let guard = state.lock().unwrap();
        let source =
            guard.file.source_file.as_ref().ok_or_else(|| "No file is loaded".to_string())?;
        match source.file_type {
            SourceFileType::SRT | SourceFileType::SSA => source.original_file.clone(),
            _ => return Err("Only subtitle files can be sent to asbplayer".to_string()),
        }
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("Couldn't read {}: {}", path, e))?;
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("subtitles.srt")
        .to_string();
    let base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    player.load_subtitles(vec![(name, base64)]).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MpvLaunchOutcome {
    Launched,
    /// `settings.mpv_path` doesn't resolve — the UI offers "Locate mpv…".
    NotFound,
}

/// Launch mpv on the IPC endpoint `MpvManager` polls (issue #89); detection
/// flips the mode to "mpv" within ~1s. Refuses while an mpv is already
/// connected — a second instance would fight over the socket.
#[tauri::command]
pub async fn launch_mpv(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
    video_path: String,
) -> Result<MpvLaunchOutcome, String> {
    if player.status().await?.mpv_connected {
        return Err("MPV is already connected".to_string());
    }
    let mpv_path = { state.lock().unwrap().settings.mpv_path.clone() };
    match std::process::Command::new(&mpv_path)
        .arg(format!("--input-ipc-server={}", yomine::mpv::default_mpv_endpoint()))
        .arg(&video_path)
        .spawn()
    {
        Ok(mut child) => {
            // Reap in the background so an exited mpv never lingers as a zombie.
            tauri::async_runtime::spawn_blocking(move || {
                let _ = child.wait();
            });
            Ok(MpvLaunchOutcome::Launched)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(MpvLaunchOutcome::NotFound),
        Err(e) => Err(format!("Failed to launch mpv ({}): {}", mpv_path, e)),
    }
}
