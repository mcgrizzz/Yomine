//! The player is owned by this task, never by the `AppState` lock:
//! `PlayerManager::update()` does blocking-ish I/O (socket reconnects, server
//! restarts), so a player tick must not stall UI commands or vice versa.
//! Commands reach it over a channel.

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
        BoundMedia,
        RemoteSubtitle,
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
    Seek {
        seconds: f32,
        label: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Status {
        reply: oneshot::Sender<PlayerStatus>,
    },
    SetPort(u16),
    /// asbplayer `get-bound-media` (issue #105). Runs on `spawn_blocking` off
    /// the task loop — the request blocks up to its timeout.
    GetBoundMedia {
        reply: oneshot::Sender<Result<Vec<BoundMedia>, String>>,
    },
    /// asbplayer `get-subtitles` for one media (all tracks when `track_numbers`
    /// is `None`).
    GetSubtitles {
        media_id: Option<String>,
        track_numbers: Option<Vec<u32>>,
        reply: oneshot::Sender<Result<Vec<RemoteSubtitle>, String>>,
    },
    /// asbplayer `mine-subtitle` (one-click mining, issue #105).
    MineSubtitle {
        fields: std::collections::HashMap<String, String>,
        post_mine_action: u8,
        reply: oneshot::Sender<Result<(), String>>,
    },
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

    pub async fn get_bound_media(&self) -> Result<Vec<BoundMedia>, String> {
        let (reply, rx) = oneshot::channel();
        self.0
            .send(PlayerCommand::GetBoundMedia { reply })
            .map_err(|_| "player task is not running".to_string())?;
        rx.await.map_err(|_| "player task dropped the request".to_string())?
    }

    pub async fn get_subtitles(
        &self,
        media_id: Option<String>,
        track_numbers: Option<Vec<u32>>,
    ) -> Result<Vec<RemoteSubtitle>, String> {
        let (reply, rx) = oneshot::channel();
        self.0
            .send(PlayerCommand::GetSubtitles { media_id, track_numbers, reply })
            .map_err(|_| "player task is not running".to_string())?;
        rx.await.map_err(|_| "player task dropped the request".to_string())?
    }

    pub async fn mine_subtitle(
        &self,
        fields: std::collections::HashMap<String, String>,
        post_mine_action: u8,
    ) -> Result<(), String> {
        let (reply, rx) = oneshot::channel();
        self.0
            .send(PlayerCommand::MineSubtitle { fields, post_mine_action, reply })
            .map_err(|_| "player task is not running".to_string())?;
        rx.await.map_err(|_| "player task dropped the request".to_string())?
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
    // Include the server's own state so the asbplayer dot can show
    // Starting/Error/Stopped, not just "waiting".
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
        // Refreshed by `player.update()` each tick, so a confirmation reaches the
        // UI within one UPDATE_INTERVAL.
        confirmed_timestamps: player.get_confirmed_timestamps(),
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
                PlayerCommand::GetBoundMedia { reply } => {
                    let server = player.ws.server.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let result = match server {
                            Some(s) => s.get_bound_media().map_err(|e| e.to_string()),
                            None => Err("WebSocket server is not running".to_string()),
                        };
                        let _ = reply.send(result);
                    });
                }
                PlayerCommand::GetSubtitles { media_id, track_numbers, reply } => {
                    let server = player.ws.server.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let result = match server {
                            Some(s) => s
                                .get_subtitles(media_id.as_deref(), track_numbers.as_deref())
                                .map_err(|e| e.to_string()),
                            None => Err("WebSocket server is not running".to_string()),
                        };
                        let _ = reply.send(result);
                    });
                }
                PlayerCommand::MineSubtitle { fields, post_mine_action, reply } => {
                    let server = player.ws.server.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let result = match server {
                            Some(s) => {
                                s.mine_subtitle(&fields, post_mine_action).map_err(|e| e.to_string())
                            }
                            None => Err("WebSocket server is not running".to_string()),
                        };
                        let _ = reply.send(result);
                    });
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
