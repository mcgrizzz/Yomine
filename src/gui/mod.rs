pub mod actions;
pub mod app;
pub mod error_modal;
pub mod file_modal;
pub mod frequency_analyzer;
pub mod message_overlay;
pub mod settings;
pub mod setup_banner;
pub mod setup_checklist_modal;
pub mod table;
pub mod theme;
pub mod top_bar;

pub use actions::{
    ActionQueue,
    UiAction,
};
pub use app::YomineApp;

// `LanguageTools` now lives in `core`; re-exported here so existing `gui::LanguageTools`
// references keep resolving.
pub use crate::core::LanguageTools;
// `recent_files` now lives in `core` (shared with the Tauri backend, which doesn't
// compile `gui`); re-exported here so `gui::recent_files::*` paths keep resolving.
pub use crate::core::recent_files;
