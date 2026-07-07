//! Player commands (T028/T035/T041, contracts/commands.md "Player"). Thin
//! wrappers over the `PlayerHandle` channel — the player itself is owned by its
//! own task (`player_task`), never the `AppState` lock. `set_websocket_port` also
//! persists the chosen port so it survives a restart (egui's websocket modal).

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

/// Persist the WebSocket port and move a running server to it (egui parity: the
/// websocket modal saves settings + restarts the server). The handle restarts a
/// live server immediately; a not-yet-started server picks up the port on its
/// next tick.
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
