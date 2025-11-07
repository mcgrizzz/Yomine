use crate::gui::table::sort::{
    SortDirection,
    SortField,
};

// A simple ui action queue system so we don't need to pass mutable references to ui functions
#[derive(Debug, Clone)]
pub enum UiAction {
    // Table State
    SetSort { field: SortField, direction: SortDirection },
    NextSentence { term_index: usize, total_sentences: usize },
    PrevSentence { term_index: usize, total_sentences: usize },
    SetFrequencyRange { min: u32, max: u32 },
    SetIncludeUnknown(bool),
    SetSearch(String),

    // Modals
    OpenPosFilters,
    OpenFrequencyWeights,

    // Ignore List
    AddToIgnoreList(String),
    RemoveFromIgnoreList(String),

    // Player
    SeekTimestamp { seconds: f32, label: String },
}

pub struct ActionQueue {
    actions: Vec<UiAction>,
}

impl ActionQueue {
    pub fn new() -> Self {
        Self { actions: Vec::new() }
    }

    pub fn push(&mut self, action: UiAction) {
        self.actions.push(action);
    }

    pub fn drain(&mut self) -> std::vec::Drain<'_, UiAction> {
        self.actions.drain(..)
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}
