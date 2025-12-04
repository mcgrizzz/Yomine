use eframe::egui;

use super::data::{
    SettingsData,
    WebSocketSettings,
};
use crate::gui::websocket_manager::WebSocketManager;

#[derive(Clone)]
pub struct WebSocketSettingsData {
    pub settings: SettingsData,
    pub temp_websocket_settings: WebSocketSettings,
    pub original_settings: SettingsData,
}

impl Default for WebSocketSettingsData {
    fn default() -> Self {
        Self {
            settings: SettingsData::default(),
            temp_websocket_settings: WebSocketSettings::default(),
            original_settings: SettingsData::default(),
        }
    }
}

impl WebSocketSettingsData {
    pub fn is_dirty(&self) -> bool {
        self.temp_websocket_settings.port != self.original_settings.websocket_settings.port
    }
}

pub struct WebSocketSettingsModal {
    open: bool,
    data: WebSocketSettingsData,
    port_input: String,
    restart_status: Option<String>,
}

impl WebSocketSettingsModal {
    pub fn new() -> Self {
        Self {
            open: false,
            data: WebSocketSettingsData::default(),
            port_input: String::new(),
            restart_status: None,
        }
    }

    pub fn open_settings(&mut self, current_settings: SettingsData) {
        self.data.settings = current_settings.clone();
        self.data.temp_websocket_settings = current_settings.websocket_settings.clone();
        self.data.original_settings = current_settings.clone();
        self.port_input = self.data.temp_websocket_settings.port.to_string();
        self.restart_status = None;
        self.open = true;
    }

    pub fn is_settings_open(&self) -> bool {
        self.open
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        websocket_manager: &mut WebSocketManager,
    ) -> Option<SettingsData> {
        if !self.open {
            return None;
        }

        let mut result: Option<SettingsData> = None;

        let modal = egui::Modal::new(egui::Id::new("websocket_settings_modal")).show(ctx, |ui| {
            ui.heading("WebSocket Server Settings");
            ui.add_space(10.0);

            self.ui_port_configuration(ui);
            ui.add_space(10.0);

            if let Some(status) = &self.restart_status {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::LIGHT_BLUE, "ℹ");
                    ui.label(status);
                });
                ui.add_space(5.0);
            }

            ui.separator();

            let is_dirty = self.data.is_dirty();

            ui.horizontal(|ui| {
                if is_dirty {
                    ui.colored_label(egui::Color32::YELLOW, "⚠");
                    ui.label("Settings have been modified");
                } else {
                    ui.colored_label(egui::Color32::TRANSPARENT, "⚠");
                    ui.label("");
                }
            });

            ui.add_space(5.0);

            ui.horizontal(|ui| {
                let save_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Save Settings")).clicked();
                let cancel_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Cancel")).clicked();

                let mut reset_clicked = false;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    reset_clicked = ui.button("Restore Default").clicked();
                });

                if save_clicked {
                    let port = self.data.temp_websocket_settings.port;
                    if self.is_valid_port(port) {
                        let mut settings = self.data.settings.clone();
                        settings.websocket_settings.port = port;

                        match websocket_manager.restart_server(port) {
                            Ok(_) => {
                                self.restart_status =
                                    Some(format!("Server restarted successfully on port {}", port));
                                self.data.original_settings = settings.clone();
                                result = Some(settings);
                            }
                            Err(e) => {
                                self.restart_status =
                                    Some(format!("Failed to restart server: {}", e));
                            }
                        }
                    } else {
                        self.restart_status =
                            Some("Invalid port range. Please use ports 1024-65535.".to_string());
                    }

                    ui.close();
                } else if cancel_clicked {
                    self.data.temp_websocket_settings =
                        self.data.original_settings.websocket_settings.clone();
                    self.data.settings = self.data.original_settings.clone();
                    self.port_input = self.data.temp_websocket_settings.port.to_string();
                    self.restart_status = None;
                } else if reset_clicked {
                    self.data.temp_websocket_settings = WebSocketSettings::default();
                    self.port_input = self.data.temp_websocket_settings.port.to_string();
                    self.restart_status = None;
                }
            });
        });

        if modal.should_close() {
            self.open = false;
        }

        result
    }

    fn ui_port_configuration(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Server Port:");

            let response = ui.add(
                egui::DragValue::new(&mut self.data.temp_websocket_settings.port)
                    .speed(1.0)
                    .range(1024..=65535)
                    .suffix(" "),
            );

            if response.changed() {
                self.port_input = self.data.temp_websocket_settings.port.to_string();
            }

            ui.label("(Valid range: 1024-65535)");
        });

        if !self.is_valid_port(self.data.temp_websocket_settings.port) {
            ui.colored_label(egui::Color32::RED, "⚠ Port must be between 1024 and 65535");
        }
    }

    fn is_valid_port(&self, port: u16) -> bool {
        port >= 1024
    }
}

impl Default for WebSocketSettingsModal {
    fn default() -> Self {
        Self::new()
    }
}
