mod background;
mod commands;
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
        .invoke_handler(tauri::generate_handler![
            commands::lifecycle::load_language_tools,
            commands::lifecycle::get_pos_catalog,
            commands::lifecycle::get_settings,
            commands::lifecycle::save_settings,
            commands::file::open_file_dialog,
            commands::file::process_file,
            commands::file::get_terms,
            commands::file::get_recent_files,
        ])
        .setup(move |app| {
            // The player runs in its own task that solely owns `PlayerManager`;
            // commands reach it through this handle (no shared lock).
            let player = player_task::spawn(app.handle().clone(), websocket_port);
            app.manage(player);

            // Ambient Anki/knowledge polling (player connectivity is handled above).
            background::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
