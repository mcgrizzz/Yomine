use eframe::egui;

use super::modal::{
    Modal,
    ModalConfig,
    ModalResult,
};

#[derive(Default, Clone)]
pub struct RestartData {
    pub message: String,
    pub requires_restart: bool,
}

pub struct RestartModal {
    modal: Modal<RestartData>,
}

impl RestartModal {
    pub fn new() -> Self {
        let config = ModalConfig {
            fixed_size: Some(egui::Vec2::new(400.0, 150.0)),
            centered: true,
            show_overlay: true,
            ..Default::default()
        };

        Self {
            modal: Modal::new_with_data("Information", RestartData::default()).with_config(config),
        }
    }

    pub fn show_restart_dialog(&mut self, message: impl Into<String>) {
        let data = RestartData { message: message.into(), requires_restart: true };
        self.modal.data = data;
        self.modal.open();
    }

    pub fn show_info_dialog(&mut self, message: impl Into<String>) {
        let data = RestartData { message: message.into(), requires_restart: false };
        self.modal.data = data;
        self.modal.open();
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        if let Some(result) = self.modal.show(ctx, |ui, data| {
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                let icon_color = if data.requires_restart {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_BLUE
                };
                let icon = if data.requires_restart { "⚠" } else { "ℹ" };

                ui.label(egui::RichText::new(icon).size(24.0).color(icon_color));
                ui.label(egui::RichText::new(&data.message).size(14.0).color(egui::Color32::WHITE));
            });

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if data.requires_restart {
                        let restart_clicked = ui.button("Restart Now").clicked();
                        let cancel_clicked = ui.button("Cancel").clicked();

                        if restart_clicked {
                            Some(ModalResult::Confirmed(data.clone()))
                        } else if cancel_clicked {
                            Some(ModalResult::Cancelled)
                        } else {
                            None
                        }
                    } else {
                        let ok_clicked = ui.button("OK").clicked();

                        if ok_clicked {
                            Some(ModalResult::Cancelled)
                        } else {
                            None
                        }
                    }
                })
                .inner
            })
            .inner
        }) {
            match result {
                ModalResult::Confirmed(_) => {
                    *self.modal.data_mut() = RestartData::default();
                    return Some(true);
                }
                ModalResult::Cancelled => {
                    *self.modal.data_mut() = RestartData::default();
                    return Some(false);
                }
                ModalResult::Custom(_, _) => {}
            }
        }

        None
    }

    pub fn is_open(&self) -> bool {
        self.modal.is_open()
    }

    pub fn close(&mut self) {
        self.modal.close();
    }
}

impl Default for RestartModal {
    fn default() -> Self {
        Self::new()
    }
}
