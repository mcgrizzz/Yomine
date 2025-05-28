use eframe::egui;
use crate::gui::file_modal::FileModal;
use crate::gui::websocket_manager::WebSocketManager;

pub struct TopBar;

impl TopBar {
    pub fn show(
        ctx: &egui::Context,
        file_modal: &mut FileModal,
        websocket_manager: &WebSocketManager,
        anki_connected: bool
    ) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_theme_preference_switch(ui);

                ui.menu_button("File", |ui| {
                    if ui.button("Open New File").clicked() {
                        file_modal.open_dialog();
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
        anki_connected: bool
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
            ui.small(egui::RichText::new("●").color(asbplayer_color)).on_hover_text(
                asbplayer_tooltip
            );
        });

        ui.add_space(3.0);

        let anki_color = if anki_connected {
            egui::Color32::from_rgb(0, 200, 0)
        } else {
            egui::Color32::from_rgb(200, 80, 80)
        };

        let anki_tooltip = if anki_connected {
            "Connected to Anki"
        } else {
            "Not Connected to Anki"
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.small("Anki").on_hover_text(anki_tooltip);
            ui.small(egui::RichText::new("●").color(anki_color)).on_hover_text(anki_tooltip);
        });
    }
}
