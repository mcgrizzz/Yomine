use eframe::egui::{self, containers};

use crate::{
    dictionary::frequency_utils,
    gui::{
        file_modal::FileModal,
        settings::{
            SettingsData,
            SettingsModal,
        },
        websocket_manager::WebSocketManager,
    },
};

pub struct TopBar;

impl TopBar {
    pub fn show(
        ctx: &egui::Context,
        file_modal: &mut FileModal,
        settings_modal: &mut SettingsModal,
        current_settings: &SettingsData,
        websocket_manager: &WebSocketManager,
        anki_connected: bool,
        restart_modal: &mut crate::gui::restart_modal::RestartModal,
    ) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            containers::menu::Bar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_switch(ui);
                ui.menu_button("File", |ui| {
                    if ui.button("Open New File").clicked() {
                        file_modal.open_dialog();
                    }
                    if ui.button("Load New Frequency Dictionaries").clicked() {
                        match frequency_utils::handle_frequency_dictionary_copy() {
                            Ok(count) => {
                                if count > 0 {
                                    println!("Successfully added {} frequency dictionaries", count);
                                    restart_modal.show_restart_dialog(
                                        format!("Successfully added {} frequency dictionaries. Please restart the application load them.", count)
                                    );
                                } else {
                                    println!("No new frequency dictionaries were selected or loaded");
                                    restart_modal.show_info_dialog(
                                        "No new frequency dictionaries were selected. No changes were made."
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to load frequency dictionaries: {}", e);
                                restart_modal.show_info_dialog(
                                    format!("Failed to load frequency dictionaries: {}", e)
                                );
                            }
                        }
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Settings", |ui| {
                    if ui.button("Anki Settings").clicked() {
                        settings_modal.open_settings(current_settings.clone(), ctx);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Self::show_status_indicators(ui, websocket_manager, anki_connected);
                });
            });
        });
    }

    fn show_status_indicators(
        ui: &mut egui::Ui,
        websocket_manager: &WebSocketManager,
        anki_connected: bool,
    ) {
        let asbplayer_connected = websocket_manager.has_clients();

        let asbplayer_color = if asbplayer_connected {
            egui::Color32::from_rgb(0, 200, 0)
        } else {
            egui::Color32::from_rgb(200, 80, 80)
        };

        let asbplayer_tooltip = if asbplayer_connected {
            "Connected to asbplayer"
        } else {
            "Not Connected to asbplayer"
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.small("asbplayer").on_hover_text(asbplayer_tooltip);
            ui.small(egui::RichText::new("●").color(asbplayer_color))
                .on_hover_text(asbplayer_tooltip);
        });

        ui.add_space(3.0);

        let anki_color = if anki_connected {
            egui::Color32::from_rgb(0, 200, 0)
        } else {
            egui::Color32::from_rgb(200, 80, 80)
        };

        let anki_tooltip =
            if anki_connected { "Connected to Anki" } else { "Not Connected to Anki" };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.small("Anki").on_hover_text(anki_tooltip);
            ui.small(egui::RichText::new("●").color(anki_color)).on_hover_text(anki_tooltip);
        });
    }
}
