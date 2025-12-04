use eframe::egui;

use crate::{
    gui::{
        settings::data::SettingsData,
        LanguageTools,
    },
    player::PlayerManager,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupItemStatus {
    Complete,
    Incomplete,
}

pub struct SetupCheckContext<'a> {
    pub language_tools: Option<&'a LanguageTools>,
    pub anki_connected: bool,
    pub player: &'a PlayerManager,
    pub settings_data: &'a SettingsData,
}

pub struct SetupCheckItem {
    pub title: String,
    pub description: String,
    pub status_fn: Box<dyn Fn(&SetupCheckContext) -> SetupItemStatus>,
    pub optional: bool,
    pub help_url: Option<String>,
    pub action: Option<SetupAction>,
    pub action_text: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SetupAction {
    OpenUrl(String),
    OpenAnkiSettings,
    LoadFrequencyDictionary,
    OpenWebSocketSettings,
}

fn check_tokenizer(ctx: &SetupCheckContext) -> SetupItemStatus {
    if ctx.language_tools.is_some() {
        SetupItemStatus::Complete
    } else {
        SetupItemStatus::Incomplete
    }
}

fn check_default_freq_dict(ctx: &SetupCheckContext) -> SetupItemStatus {
    ctx.language_tools
        .and_then(|lt| {
            let manager = &lt.frequency_manager;

            let has_dict = !manager.get_dictionary_names().is_empty();
            if has_dict {
                Some(SetupItemStatus::Complete)
            } else {
                Some(SetupItemStatus::Incomplete)
            }
        })
        .unwrap_or(SetupItemStatus::Incomplete)
}

fn check_additional_freq_dicts(ctx: &SetupCheckContext) -> SetupItemStatus {
    ctx.language_tools
        .and_then(|lt| {
            let manager = &lt.frequency_manager;

            let dict_count = manager.get_dictionary_names().len();
            if dict_count > 1 {
                Some(SetupItemStatus::Complete)
            } else {
                Some(SetupItemStatus::Incomplete)
            }
        })
        .unwrap_or(SetupItemStatus::Incomplete)
}

fn check_anki_connect(ctx: &SetupCheckContext) -> SetupItemStatus {
    if ctx.anki_connected {
        SetupItemStatus::Complete
    } else {
        SetupItemStatus::Incomplete
    }
}

fn check_anki_models(ctx: &SetupCheckContext) -> SetupItemStatus {
    if !ctx.settings_data.anki_model_mappings.is_empty() {
        SetupItemStatus::Complete
    } else {
        SetupItemStatus::Incomplete
    }
}

fn check_player_detected(ctx: &SetupCheckContext) -> SetupItemStatus {
    let mpv_connected = ctx.player.mpv.is_connected();
    let ws_connected = ctx.player.ws.has_clients();

    if mpv_connected || ws_connected {
        SetupItemStatus::Complete
    } else {
        SetupItemStatus::Incomplete
    }
}

pub struct SetupChecklistModal {
    open: bool,
    items: Vec<SetupCheckItem>,
}

impl SetupChecklistModal {
    pub fn new() -> Self {
        let items = vec![
            SetupCheckItem {
                title: "Tokenizer Installed".to_string(),
                description: "Required for Japanese text segmentation".to_string(),
                status_fn: Box::new(check_tokenizer),
                optional: false,
                help_url: None,
                action: None,
                action_text: None,
            },
            SetupCheckItem {
                title: "Default Frequency Dictionary Installed".to_string(),
                description: "Auto-downloads on first run".to_string(),
                status_fn: Box::new(check_default_freq_dict),
                optional: false,
                help_url: None,
                action: Some(SetupAction::LoadFrequencyDictionary),
                action_text: Some("+ Install Dictionary".to_string()),
            },
            SetupCheckItem {
                title: "AnkiConnect Enabled and Detected".to_string(),
                description: "Required for Anki integration".to_string(),
                status_fn: Box::new(check_anki_connect),
                optional: false,
                help_url: Some("https://ankiweb.net/shared/info/2055492159".to_string()),
                action: None,
                action_text: None,
            },
            SetupCheckItem {
                title: "Anki Notetypes Setup".to_string(),
                description: "Required for Anki integration".to_string(),
                status_fn: Box::new(check_anki_models),
                optional: false,
                help_url: Some("https://github.com/mcgrizzz/Yomine?tab=readme-ov-file#setting-up-anki-integration".to_string()),
                action: Some(SetupAction::OpenAnkiSettings),
                action_text: Some("Setup Anki".to_string()),
            },
            SetupCheckItem {
                title: "asbplayer or mpv detected".to_string(),
                description: "Required for video timestamp integration".to_string(),
                status_fn: Box::new(check_player_detected),
                optional: false,
                help_url: Some("https://github.com/mcgrizzz/Yomine?tab=readme-ov-file#configuring-websocket-connection".to_string()),
                action: Some(SetupAction::OpenWebSocketSettings),
                action_text: Some("Configure WebSocket".to_string()),
            },
            SetupCheckItem {
                title: "Additional Frequency Dictionaries Installed [Optional]".to_string(),
                description: "Load additional dictionaries via File menu".to_string(),
                status_fn: Box::new(check_additional_freq_dicts),
                optional: true,
                help_url: Some("https://github.com/mcgrizzz/Yomine?tab=readme-ov-file#setting-up-frequency-dictionaries".to_string()),
                action: Some(SetupAction::LoadFrequencyDictionary),
                action_text: Some("+ Install Dictionary".to_string()),
            },
        ];

        Self { open: false, items }
    }

    pub fn open_modal(&mut self) {
        self.open = true;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        language_tools: Option<&LanguageTools>,
        anki_connected: bool,
        player: &PlayerManager,
        settings_data: &SettingsData,
    ) -> Option<SetupAction> {
        if !self.open {
            return None;
        }

        let mut action_to_return: Option<SetupAction> = None;

        let check_context =
            SetupCheckContext { language_tools, anki_connected, player, settings_data };

        let modal = egui::Modal::new(egui::Id::new("setup_checklist_modal")).show(ctx, |ui| {
            ui.set_width(600.0);

            ui.add_space(10.0);
            ui.heading("Setup Checklist");
            ui.add_space(15.0);

            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                for item in &self.items {
                    let status = (item.status_fn)(&check_context);

                    ui.horizontal(|ui| {
                        let (icon, icon_color) = match (status, item.optional) {
                            (SetupItemStatus::Complete, _) => {
                                ("✓", egui::Color32::from_rgb(0, 200, 0))
                            }
                            (SetupItemStatus::Incomplete, false) => {
                                ("x", egui::Color32::from_rgb(200, 80, 80))
                            }
                            (SetupItemStatus::Incomplete, true) => {
                                ("◯", egui::Color32::from_rgb(150, 150, 150))
                            }
                        };

                        ui.label(egui::RichText::new(icon).size(20.0).color(icon_color));

                        ui.vertical(|ui| {
                            let title_color = match (status, item.optional) {
                                (SetupItemStatus::Complete, _) => {
                                    egui::Color32::from_rgb(0, 200, 0) // Green
                                }
                                (SetupItemStatus::Incomplete, false) => {
                                    egui::Color32::from_rgb(200, 80, 80) // Red
                                }
                                (SetupItemStatus::Incomplete, true) => {
                                    egui::Color32::from_rgb(150, 150, 150) // Gray
                                }
                            };

                            ui.label(egui::RichText::new(&item.title).strong().color(title_color));
                            ui.label(
                                egui::RichText::new(&item.description)
                                    .size(12.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(url) = &item.help_url {
                                if ui.button("📖 View Docs").clicked() {
                                    action_to_return = Some(SetupAction::OpenUrl(url.clone()));
                                }
                            }

                            if let Some(action) = &item.action {
                                let button_text =
                                    item.action_text.as_deref().unwrap_or("⚙ Configure");
                                if ui.button(button_text).clicked() {
                                    action_to_return = Some(action.clone());
                                    ui.close();
                                }
                            }
                        });
                    });

                    ui.add_space(10.0);
                }
            });

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        ui.close();
                    }
                });
            });
        });

        if modal.should_close() {
            self.open = false;
        }

        action_to_return
    }
}
