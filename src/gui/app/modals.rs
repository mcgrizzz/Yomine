use crate::gui::{
    error_modal::ErrorModal,
    file_modal::FileModal,
    frequency_analyzer::FrequencyAnalyzerModal,
    settings::{
        AnkiSettingsModal,
        FrequencyWeightsModal,
        IgnoreListModal,
        PosFiltersModal,
        WebSocketSettingsModal,
    },
    setup_checklist_modal::SetupChecklistModal,
};

pub struct Modals {
    pub file: FileModal,
    pub error: ErrorModal,
    pub anki_settings: AnkiSettingsModal,
    pub websocket_settings: WebSocketSettingsModal,
    pub ignore_list: IgnoreListModal,
    pub frequency_weights: FrequencyWeightsModal,
    pub pos_filters: PosFiltersModal,
    pub frequency_analyzer: FrequencyAnalyzerModal,
    pub setup_checklist: SetupChecklistModal,
}

impl Default for Modals {
    fn default() -> Self {
        Self {
            file: FileModal::new(),
            error: ErrorModal::new(),
            anki_settings: AnkiSettingsModal::new(),
            websocket_settings: WebSocketSettingsModal::new(),
            ignore_list: IgnoreListModal::new(),
            frequency_weights: FrequencyWeightsModal::new(),
            pos_filters: PosFiltersModal::new(),
            frequency_analyzer: FrequencyAnalyzerModal::new(),
            setup_checklist: SetupChecklistModal::new(),
        }
    }
}
