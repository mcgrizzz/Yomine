//! One-shot pull of the cached summary — the event fires only on change, so a
//! (re)loaded webview would otherwise sit blank until the next recompute.

use std::sync::Mutex;

use tauri::State;

use crate::{
    dto::KnowledgeSummaryDto,
    state::AppState,
};

#[tauri::command]
pub fn get_knowledge_summary(state: State<'_, Mutex<AppState>>) -> Option<KnowledgeSummaryDto> {
    state.lock().unwrap().knowledge_summary.clone()
}
