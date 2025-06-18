use eframe::egui;

#[derive(Default, Clone)]
pub struct RestartData {
    pub message: String,
    pub requires_restart: bool,
}

pub struct RestartModal {
    open: bool,
    data: RestartData,
}

impl RestartModal {
    pub fn new() -> Self {
        Self { open: false, data: RestartData::default() }
    }

    pub fn show_restart_dialog(&mut self, message: impl Into<String>) {
        self.data = RestartData { message: message.into(), requires_restart: true };
        self.open = true;
    }

    pub fn show_info_dialog(&mut self, message: impl Into<String>) {
        self.data = RestartData { message: message.into(), requires_restart: false };
        self.open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        if !self.open {
            return None;
        }

        let mut result: Option<bool> = None;

        let modal = egui::Modal::new(egui::Id::new("restart_modal")).show(ctx, |ui| {
            ui.set_width(400.0);

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                let icon_color = if self.data.requires_restart {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_BLUE
                };
                let icon = if self.data.requires_restart { "⚠" } else { "ℹ" };

                ui.label(egui::RichText::new(icon).size(24.0).color(icon_color));
                ui.label(
                    egui::RichText::new(&self.data.message).size(14.0).color(egui::Color32::WHITE),
                );
            });

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.data.requires_restart {
                        if ui.button("Restart Now").clicked() {
                            result = Some(true);
                            ui.close();
                        }

                        if ui.button("Cancel").clicked() {
                            result = Some(false);
                            ui.close();
                        }
                    } else {
                        if ui.button("OK").clicked() {
                            result = Some(false);
                            ui.close();
                        }
                    }
                });
            });
        });

        if modal.should_close() {
            self.open = false;
            self.data = RestartData::default();
        }

        result
    }
}

impl Default for RestartModal {
    fn default() -> Self {
        Self::new()
    }
}
