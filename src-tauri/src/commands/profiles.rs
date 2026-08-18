//! Switching is a relaunch driven by the frontend, so none of these touch `AppState`.

use tauri::{
    AppHandle,
    Manager,
};
use yomine::persistence::profiles;

use crate::dto::{
    ProfileDto,
    ProfileListDto,
    ProfileMutationDto,
};

#[tauri::command]
pub fn list_profiles() -> ProfileListDto {
    ProfileListDto {
        profiles: profiles::list()
            .into_iter()
            .map(|p| ProfileDto { slug: p.slug, display_name: p.display_name, active: p.active })
            .collect(),
    }
}

#[tauri::command]
pub fn create_profile(name: String) -> Result<ProfileMutationDto, String> {
    profiles::create(&name).map_err(|e| e.to_string())?;
    Ok(ProfileMutationDto { requires_relaunch: true })
}

#[tauri::command]
pub fn copy_profile(slug: String, name: String) -> Result<ProfileMutationDto, String> {
    profiles::copy(&slug, &name).map_err(|e| e.to_string())?;
    Ok(ProfileMutationDto { requires_relaunch: true })
}

#[tauri::command]
pub fn switch_profile(slug: String) -> Result<ProfileMutationDto, String> {
    profiles::switch(&slug).map_err(|e| e.to_string())?;
    Ok(ProfileMutationDto { requires_relaunch: true })
}

#[tauri::command]
pub fn rename_profile(app: AppHandle, slug: String, name: String) -> Result<(), String> {
    profiles::rename(&slug, &name).map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window("main") {
        let title = profiles::active_title_name()
            .map_or_else(|| "Yomine".to_string(), |n| format!("Yomine - {n}"));
        let _ = window.set_title(&title);
    }
    Ok(())
}

#[tauri::command]
pub fn delete_profile(slug: String) -> Result<ProfileMutationDto, String> {
    let was_active = profiles::delete(&slug).map_err(|e| e.to_string())?;
    Ok(ProfileMutationDto { requires_relaunch: was_active })
}
