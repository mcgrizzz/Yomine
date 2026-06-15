//! The player lives here, owned solely by one background task — never in the
//! shared `AppState` lock. `PlayerManager::update()` does blocking-ish I/O on a
//! timer (socket reconnects, WebSocket server restart); keeping it out of the
//! state mutex means a player tick never stalls UI commands and vice versa.
//!
//! Commands talk to it over a channel: `seek`/`status` carry a `oneshot` reply;
//! `set_port` is fire-and-forget. The task also emits `player-status` whenever
//! connectivity or mode changes (replaces egui's per-frame `PlayerManager::update`).

use std::time::Duration;

use tauri::{
    AppHandle,
    Emitter,
};
use tokio::sync::{
    mpsc,
    oneshot,
};
use yomine::{
    mpv::MpvManager,
    player::PlayerManager,
    websocket::{
        ServerState,
        WebSocketManager,
    },
};

use crate::events::{
    names,
    PlayerStatus,
};

/// How often the task polls player connectivity. Seeks are immediate (channel),
/// so this only bounds how fast a connect/disconnect is reflected in the UI.
const UPDATE_INTERVAL: Duration = Duration::from_millis(250);

pub enum PlayerCommand {
    Seek { seconds: f32, label: String, reply: oneshot::Sender<Result<(), String>> },
    Status { reply: oneshot::Sender<PlayerStatus> },
    SetPort(u16),
}

/// Cheap-to-clone handle commands hold to reach the player task.
#[derive(Clone)]
pub struct PlayerHandle(mpsc::UnboundedSender<PlayerCommand>);

impl PlayerHandle {
    pub async fn seek(&self, seconds: f32, label: String) -> Result<(), String> {
        let (reply, rx) = oneshot::channel();
        self.0
            .send(PlayerCommand::Seek { seconds, label, reply })
            .map_err(|_| "player task is not running".to_string())?;
        rx.await.map_err(|_| "player task dropped the seek request".to_string())?
    }

    pub async fn status(&self) -> Result<PlayerStatus, String> {
        let (reply, rx) = oneshot::channel();
        self.0
            .send(PlayerCommand::Status { reply })
            .map_err(|_| "player task is not running".to_string())?;
        rx.await.map_err(|_| "player task dropped the status request".to_string())
    }

    pub fn set_port(&self, port: u16) {
        let _ = self.0.send(PlayerCommand::SetPort(port));
    }
}

/// Spawn the player task and return a handle to it. Call once at app setup.
pub fn spawn(app: AppHandle, websocket_port: u16) -> PlayerHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    tauri::async_runtime::spawn(run(app, websocket_port, rx));
    PlayerHandle(tx)
}

fn current_status(player: &PlayerManager) -> PlayerStatus {
    let mpv_connected = player.mpv.is_connected();
    let has_clients = player.ws.has_clients();
    let mode = if mpv_connected {
        "mpv"
    } else if player.ws.server.is_some() {
        "asbplayer"
    } else {
        "none"
    };
    // Carry the WebSocket server's own state so the asbplayer dot can show
    // Starting/Error/Stopped, not just "waiting" (T056; mirrors egui's
    // `show_status_indicators` reading `WebSocketManager::get_server_state`).
    let (server_state, server_error) = match player.ws.get_server_state() {
        ServerState::Running => ("running", None),
        ServerState::Starting => ("starting", None),
        ServerState::Stopped => ("stopped", None),
        ServerState::Error(msg) => ("error", Some(msg)),
    };
    PlayerStatus {
        mpv_connected,
        // The engine tracks presence (bool), not a precise count.
        ws_clients: if has_clients { 1 } else { 0 },
        mode: mode.to_string(),
        server_state: server_state.to_string(),
        server_error,
    }
}

async fn run(app: AppHandle, mut port: u16, mut rx: mpsc::UnboundedReceiver<PlayerCommand>) {
    let mpv = MpvManager::new();
    let ws = WebSocketManager::new(port);
    let mut player = PlayerManager::new(mpv, ws);

    let mut last_status: Option<PlayerStatus> = None;
    let mut tick = tokio::time::interval(UPDATE_INTERVAL);

    loop {
        tokio::select! {
            _ = tick.tick() => {
                player.update(port);
                let status = current_status(&player);
                if last_status.as_ref() != Some(&status) {
                    let _ = app.emit(names::PLAYER_STATUS, status.clone());
                    last_status = Some(status);
                }
            }
            Some(cmd) = rx.recv() => match cmd {
                PlayerCommand::Seek { seconds, label, reply } => {
                    let result = player.seek_timestamp(seconds, &label).map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
                PlayerCommand::Status { reply } => {
                    let _ = reply.send(current_status(&player));
                }
                PlayerCommand::SetPort(new_port) => {
                    if port != new_port {
                        port = new_port;
                        // Move a running server to the new port now; if none is
                        // running, the next `update()` tick uses the new port.
                        if player.ws.server.is_some() {
                            if let Err(e) = player.ws.restart_server(port) {
                                eprintln!("[Player] Failed to restart WebSocket server on port {}: {}", port, e);
                            }
                        }
                    }
                }
            }
        }
    }
}
