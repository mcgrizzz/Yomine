use eframe::egui;

use super::modal::{
    Modal,
    ModalConfig,
    ModalResult,
};

#[derive(Default, Clone)]
pub struct ErrorData {
    pub title: String,
    pub message: String,
    pub details: Option<String>,
}

pub struct ErrorModal {
    modal: Modal<ErrorData>,
}

impl ErrorModal {
    pub fn new() -> Self {
        let config = ModalConfig {
            fixed_size: Some(egui::Vec2::new(450.0, 250.0)),
            centered: true,
            show_overlay: true,
            resizable: true,
            ..Default::default()
        };

        Self { modal: Modal::new_with_data("Error", ErrorData::default()).with_config(config) }
    }

    pub fn show_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        let data = ErrorData { title: title.into(), message: message.into(), details: None };
        self.modal.data = data;
        self.modal.title = "Error".to_string();
        self.modal.open();
    }

    pub fn show_error_with_details(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) {
        let data = ErrorData {
            title: title.into(),
            message: message.into(),
            details: Some(details.into()),
        };
        self.modal.data = data;
        self.modal.title = "Error".to_string();
        self.modal.open();
    }
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if let Some(result) = self.modal.show(ctx, |ui, data| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚠").size(24.0).color(egui::Color32::RED));
                ui.label(
                    egui::RichText::new(&data.title)
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
            });

            ui.add_space(10.0);

            ui.label(
                egui::RichText::new(&data.message).size(14.0).color(egui::Color32::LIGHT_GRAY),
            );

            if let Some(details) = &data.details {
                ui.add_space(10.0);
                ui.collapsing("Technical Details", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut details.as_str())
                            .desired_width(f32::INFINITY)
                            .desired_rows(4)
                            .code_editor(),
                    );
                });
            }

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        Some(ModalResult::Confirmed(data.clone()))
                    } else {
                        None
                    }
                })
                .inner
            })
            .inner
        }) {
            match result {
                ModalResult::Confirmed(_) => {
                    *self.modal.data_mut() = ErrorData::default();
                    return true;
                }
                ModalResult::Cancelled => {
                    *self.modal.data_mut() = ErrorData::default();
                    return true;
                }
                ModalResult::Custom(_, _) => {}
            }
        }

        false
    }

    pub fn is_open(&self) -> bool {
        self.modal.is_open()
    }

    pub fn close(&mut self) {
        self.modal.close();
    }
}

impl Default for ErrorModal {
    fn default() -> Self {
        Self::new()
    }
}
