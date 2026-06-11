//! Setup-status command (T028, contracts/commands.md "Misc"). Aggregates the
//! readiness signals the setup checklist/banner shows. Each field mirrors the
//! matching egui `check_*` in `gui::setup_checklist_modal`.

use std::sync::Mutex;

use tauri::State;
use yomine::anki;

use crate::{
    dto::SetupStatus,
    player_task::PlayerHandle,
    state::AppState,
};

/// Snapshot of setup readiness. The lock is taken only for the in-state bits
/// (tools/mapping/dict); the Anki and player probes happen unlocked.
#[tauri::command]
pub async fn get_setup_status(
    state: State<'_, Mutex<AppState>>,
    player: State<'_, PlayerHandle>,
) -> Result<SetupStatus, String> {
    let (tools_loaded, has_field_mapping, has_frequency_dict) = {
        let guard = state.lock().unwrap();
        let tools_loaded = guard.language_tools.is_some();
        let has_field_mapping = !guard.settings.anki_model_mappings.is_empty();
        let has_frequency_dict = guard
            .language_tools
            .as_ref()
            .is_some_and(|t| !t.frequency_manager.get_dictionary_names().is_empty());
        (tools_loaded, has_field_mapping, has_frequency_dict)
    };

    let anki_connected = anki::api::get_version().await.is_ok();
    let player = player.status().await?;
    let player_connected = player.mpv_connected || player.ws_clients > 0;

    Ok(SetupStatus {
        tools_loaded,
        anki_connected,
        has_field_mapping,
        has_frequency_dict,
        player_connected,
    })
}
