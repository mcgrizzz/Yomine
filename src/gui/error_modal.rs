use eframe::egui;

#[derive(Default, Clone)]
pub struct ErrorData {
    pub title: String,
    pub message: String,
    pub details: Option<String>,
}

pub struct ErrorModal {
    open: bool,
    data: ErrorData,
}

impl ErrorModal {
    pub fn new() -> Self {
        Self { open: false, data: ErrorData::default() }
    }

    pub fn show_error(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        details: Option<impl Into<String>>,
    ) {
        self.data = ErrorData {
            title: title.into(),
            message: message.into(),
            details: details.map(|d| d.into()),
        };

        self.open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if self.open {
            let modal = egui::Modal::new(egui::Id::new("error_modal")).show(ctx, |ui| {
                ui.set_width(450.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚠").size(24.0).color(egui::Color32::RED));
                    ui.label(
                        egui::RichText::new(&self.data.title)
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                });

                ui.add_space(10.0);

                ui.label(
                    egui::RichText::new(&self.data.message)
                        .size(14.0)
                        .color(egui::Color32::LIGHT_GRAY),
                );

                if let Some(details) = &self.data.details {
                    ui.add_space(10.0);
                    ui.collapsing("Technical Details", |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut details.as_str())
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .code_editor(),
                        );
                    });
                };

                ui.add_space(15.0);

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("OK").clicked() {
                            ui.close();
                        }
                    });
                });
            });

            if modal.should_close() {
                self.open = false;
                self.data = ErrorData::default();
                return true;
            }
        }

        false
    }
}

impl Default for ErrorModal {
    fn default() -> Self {
        Self::new()
    }
}
