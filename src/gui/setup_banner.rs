use eframe::egui;

use crate::gui::{
    settings::data::SettingsData,
    LanguageTools,
};

pub struct SetupBanner;

impl SetupBanner {
    pub fn show(
        ctx: &egui::Context,
        language_tools: Option<&LanguageTools>,
        settings_data: &SettingsData,
    ) -> bool {
        if !Self::should_show_banner(language_tools, settings_data) {
            return false;
        }

        let mut clicked = false;

        egui::TopBottomPanel::top("setup_banner").exact_height(30.0).show(ctx, |ui| {
            let banner_color = egui::Color32::from_rgb(180, 140, 0);
            let frame = egui::Frame::NONE.fill(banner_color);

            frame.show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    let text = "⚠ Setup Incomplete - Click to view setup checklist";
                    let response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(text).size(14.0).color(egui::Color32::WHITE),
                        )
                        .sense(egui::Sense::click()),
                    );

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if response.clicked() {
                        clicked = true;
                    }
                });
            });
        });

        clicked
    }

    fn should_show_banner(
        language_tools: Option<&LanguageTools>,
        settings_data: &SettingsData,
    ) -> bool {
        let freq_dict_missing = language_tools
            .map(|lt| lt.frequency_manager.get_dictionary_names().is_empty())
            .unwrap_or(true);

        let anki_models_missing = settings_data.anki_model_mappings.is_empty();

        freq_dict_missing || anki_models_missing
    }
}
