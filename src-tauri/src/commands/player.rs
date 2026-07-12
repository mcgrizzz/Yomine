//! Player commands (contracts/commands.md "Player"): thin wrappers over the
//! `PlayerHandle` channel — the player is owned by its task, never this lock.

use std::sync::Mutex;

use tauri::State;
use yomine::persistence;

use crate::{
    events::PlayerStatus,
    player_task::PlayerHandle,
    state::AppState,
};

/// Seek the active player to `seconds` (prefers MPV, else the WebSocket client).
/// Errors if no player is connected (the handle relays the player's own error).
#[tauri::command]
pub async fn seek_timestamp(
    player: State<'_, PlayerHandle>,
    seconds: f32,
    label: String,
) -> Result<(), String> {
    player.seek(seconds, label).await
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
