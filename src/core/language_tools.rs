use std::sync::{
    Arc,
    Mutex,
};

use vibrato::Tokenizer;

use crate::{
    core::IgnoreList,
    dictionary::frequency_manager::FrequencyManager,
};

/// Shared, heavy language resources loaded once and reused across the app.
///
/// This lives in `core` (not a UI layer) so both the egui frontend and the Tauri
/// backend can hold it. None of these fields are serialized; they are kept behind
/// `Arc` and handed to the analysis pipeline.
#[derive(Clone)]
pub struct LanguageTools {
    pub tokenizer: Arc<Tokenizer>,
    pub frequency_manager: Arc<FrequencyManager>,
    pub ignore_list: Arc<Mutex<IgnoreList>>,
    pub known_interval: u32,
}

impl std::fmt::Debug for LanguageTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageTools")
            .field("tokenizer", &"Arc<Tokenizer>")
            .field("frequency_manager", &"Arc<FrequencyManager>")
            .field("ignore_list", &"Arc<Mutex<IgnoreList>>")
            .finish()
    }
}
