mod dto;
mod events;
mod player_task;
mod state;

use std::sync::Mutex;

use tauri::Manager;
use yomine::core::settings::SettingsData;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Settings are owned by the backend (Constitution: one source of truth) and
    // loaded from the same `settings.json` the egui app uses — unchanged on-disk
    // format means existing users' settings load in both apps (research R9).
    let settings = yomine::persistence::load_json_or_default::<SettingsData>("settings.json");
    let websocket_port = settings.websocket_settings.port;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(AppState::new(settings)))
        .setup(move |app| {
            // The player runs in its own task that solely owns `PlayerManager`;
            // commands reach it through this handle (no shared lock).
            let player = player_task::spawn(app.handle().clone(), websocket_port);
            app.manage(player);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
