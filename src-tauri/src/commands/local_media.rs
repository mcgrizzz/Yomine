//! ffmpeg for local mining.

use std::path::Path;

use tauri::ipc::Channel;
use yomine::media::{
    download,
    ffmpeg,
};

use crate::events::LoadingMessage;

#[derive(serde::Serialize)]
pub struct FfmpegStatus {
    /// The ffmpeg local mining would run, if any.
    path: Option<String>,
    /// Whether Yomine can download a build for this system.
    downloadable: bool,
}

/// `configured` is the settings value, which may be unsaved.
#[tauri::command]
pub async fn get_ffmpeg_status(configured: String) -> FfmpegStatus {
    let path = tauri::async_runtime::spawn_blocking(move || {
        let configured = Some(configured.trim()).filter(|p| !p.is_empty()).map(Path::new);
        ffmpeg::find(configured).map(|f| f.path().display().to_string())
    })
    .await
    .ok()
    .flatten();
    FfmpegStatus { path, downloadable: download::available() }
}

#[tauri::command]
pub async fn install_ffmpeg(progress: Channel<LoadingMessage>) -> Result<String, String> {
    let path = tauri::async_runtime::spawn_blocking(move || {
        let report = |message: String| {
            let _ = progress.send(LoadingMessage::new(message));
        };
        let result = download::install(Some(&report));
        let _ = progress.send(LoadingMessage::clear());
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}
