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
            commands::file::refresh_terms,
            commands::file::get_recent_files,
            commands::ignore::get_ignore_list,
            commands::ignore::add_to_ignore_list,
            commands::ignore::remove_from_ignore_list,
            commands::ignore::get_ignore_list_full,
            commands::ignore::import_ignore_file,
            commands::ignore::refresh_ignore_file,
            commands::ignore::save_ignore_list,
            commands::ignore::get_default_ignored_terms,
            commands::ignore::export_ignore_list,
            commands::anki::get_anki_status,
            commands::anki::list_anki_models,
            commands::anki::get_anki_sample_note,
            commands::dictionary::list_dictionaries,
            commands::dictionary::set_dictionary_state,
            commands::analysis::find_analysis_files,
            commands::analysis::start_analysis,
            commands::analysis::cancel_analysis,
            commands::analysis::export_analysis,
            commands::player::seek_timestamp,
            commands::player::get_player_status,
            commands::player::set_websocket_port,
            commands::setup::get_setup_status,
            commands::knowledge::get_knowledge_summary,
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
