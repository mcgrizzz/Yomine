//! Aggregates the readiness signals the setup checklist/banner shows.

use std::sync::Mutex;

use tauri::State;
use yomine::{
    anki,
    core::settings::MiningMode,
    media::ffmpeg,
};

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
    let (tools_loaded, has_field_mapping, frequency_dict_count, yomitan_url, mode, ffmpeg_path) = {
        let guard = state.lock().unwrap();
        let tools_loaded = guard.language_tools.is_some();
        let has_field_mapping = !guard.settings.anki_model_mappings.is_empty();
        let frequency_dict_count = guard
            .language_tools
            .as_ref()
            .map_or(0, |t| t.frequency_manager.get_dictionary_names().len());
        (
            tools_loaded,
            has_field_mapping,
            frequency_dict_count,
            guard.settings.yomitan_url.clone(),
            guard.settings.mining_mode,
            guard.settings.ffmpeg_path.clone(),
        )
    };
    let has_frequency_dict = frequency_dict_count > 0;

    let anki_connected = anki::api::get_version().await.is_ok();
    let yomitan_connected = yomine::yomitan::get_version(&yomitan_url).await.is_ok();
    let media_ready = match mode {
        MiningMode::Asbplayer => player.status().await?.ws_clients > 0,
        MiningMode::Local => tauri::async_runtime::spawn_blocking(move || {
            let configured = Some(ffmpeg_path.trim()).filter(|p| !p.is_empty());
            ffmpeg::find(configured.map(std::path::Path::new)).is_some()
        })
        .await
        .unwrap_or(false),
    };

    Ok(SetupStatus {
        tools_loaded,
        anki_connected,
        has_field_mapping,
        has_frequency_dict,
        frequency_dict_count,
        media_ready,
        yomitan_connected,
    })
}
