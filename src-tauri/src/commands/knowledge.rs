//! Knowledge-summary pull (US7/T049). The summary is computed by the background
//! task and emitted on `knowledge-summary` only when an input changes; a freshly
//! (re)loaded webview pulls the last cached value here so the widget isn't blank
//! until the next recompute (same rationale as the anki/player status pull in
//! `hydrate`).

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
