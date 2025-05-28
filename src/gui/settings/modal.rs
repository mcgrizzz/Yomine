use std::collections::HashMap;

use eframe::egui;

use super::{
    anki_service::AnkiService,
    components::{
        connection_status_ui,
        field_selection_ui,
        model_selection_ui,
    },
    data::{
        AnkiModelInfo,
        ModelMappingEditor,
        SettingsData,
    },
};
use crate::{
    anki::FieldMapping,
    gui::modal::{
        Modal,
        ModalConfig,
        ModalResult,
    },
};

#[derive(Clone)]
pub struct SettingsModalData {
    pub settings: SettingsData,
    pub temp_model_mappings: HashMap<String, FieldMapping>,
    pub original_settings: SettingsData,
}

impl Default for SettingsModalData {
    fn default() -> Self {
        Self {
            settings: SettingsData::new(),
            temp_model_mappings: HashMap::new(),
            original_settings: SettingsData::new(),
        }
    }
}

impl SettingsModalData {
    pub fn is_dirty(&self) -> bool {
        self.temp_model_mappings != self.original_settings.anki_model_mappings
    }
}

pub struct SettingsModal {
    modal: Modal<SettingsModalData>,
    model_editor: ModelMappingEditor,
    available_models: Vec<AnkiModelInfo>,
    anki_service: AnkiService,
}

impl SettingsModal {
    pub fn new() -> Self {
        let config = ModalConfig {
            min_size: Some(egui::Vec2::new(700.0, 500.0)),
            fixed_size: Some(egui::Vec2::new(750.0, 550.0)),
            centered: true,
            show_overlay: true,
            resizable: true,
            ..Default::default()
        };

        Self {
            modal: Modal::new_with_data("Anki Model Mappings", SettingsModalData::default())
                .with_config(config),
            model_editor: ModelMappingEditor::default(),
            available_models: Vec::new(),
            anki_service: AnkiService::new(),
        }
    }

    pub fn open_settings(&mut self, current_settings: SettingsData, ctx: &egui::Context) {
        self.modal.data_mut().settings = current_settings.clone();
        self.modal.data_mut().temp_model_mappings = current_settings.anki_model_mappings.clone();
        self.modal.data_mut().original_settings = current_settings.clone();
        self.modal.open();

        if self.available_models.is_empty() {
            self.anki_service.fetch_models(ctx);
        }

        for model_name in current_settings.anki_model_mappings.keys() {
            let has_sample = self
                .available_models
                .iter()
                .find(|m| m.name == *model_name)
                .map(|m| m.sample_note.is_some())
                .unwrap_or(false);

            if !has_sample && !self.anki_service.is_loading_models {
                self.anki_service.fetch_sample_note(model_name, ctx);
            }
        }
    }

    pub fn is_settings_open(&self) -> bool {
        self.modal.is_open()
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<SettingsData> {
        self.anki_service.check_async_results(&mut self.available_models, ctx);

        let available_models = self.available_models.clone();
        let mut anki_service = std::mem::take(&mut self.anki_service);
        let mut model_editor = std::mem::take(&mut self.model_editor);

        let result = self.modal.show(ctx, |ui, data| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                show_existing_mappings_ui(ui, data, &mut model_editor);
                ui.separator();

                ui.heading(if model_editor.is_editing {
                    "Edit Model Mapping"
                } else {
                    "Add Model Mapping"
                });
                ui.add_space(1.0);
                connection_status_ui(ui, &mut anki_service, ctx);
                ui.add_space(5.0);

                show_mapping_editor_ui(
                    ui,
                    ctx,
                    &mut model_editor,
                    &available_models,
                    &mut anki_service,
                    data,
                );
            });

            ui.separator();

            let is_dirty = data.is_dirty();
            if is_dirty {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::YELLOW, "⚠");
                    ui.label("Settings have been modified");
                });
                ui.add_space(5.0);
            }

            ui.horizontal(|ui| {
                let is_dirty = data.is_dirty();

                let save_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Save Settings")).clicked();
                let cancel_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Cancel")).clicked();
                let close_clicked = ui.button("Close Settings").clicked();

                let mut reset_clicked = false;
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    reset_clicked = ui.button("Restore Default").clicked();
                });

                if save_clicked {
                    let mut settings = data.settings.clone();
                    settings.anki_model_mappings = data.temp_model_mappings.clone();
                    data.original_settings = settings.clone();
                    Some(ModalResult::Custom("save".to_string(), data.clone()))
                } else if cancel_clicked {
                    data.temp_model_mappings = data.original_settings.anki_model_mappings.clone();
                    data.settings = data.original_settings.clone();
                    None
                } else if reset_clicked {
                    data.temp_model_mappings.clear();
                    data.settings = SettingsData::new();
                    None
                } else if close_clicked {
                    Some(ModalResult::Cancelled)
                } else {
                    None
                }
            })
            .inner
        });

        self.anki_service = anki_service;
        self.model_editor = model_editor;

        if let Some(modal_result) = result {
            match modal_result {
                ModalResult::Confirmed(data) => {
                    let mut settings = data.settings.clone();
                    settings.anki_model_mappings = data.temp_model_mappings.clone();
                    return Some(settings);
                }
                ModalResult::Custom(action, data) if action == "save" => {
                    let mut settings = data.settings.clone();
                    settings.anki_model_mappings = data.temp_model_mappings.clone();
                    return Some(settings);
                }
                ModalResult::Cancelled => return None,
                _ => {}
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

impl Default for SettingsModal {
    fn default() -> Self {
        Self::new()
    }
}

fn show_existing_mappings_ui(
    ui: &mut egui::Ui,
    data: &mut SettingsModalData,
    model_editor: &mut ModelMappingEditor,
) {
    ui.heading("Current Model Mappings");
    ui.add_space(5.0);

    let mut to_remove = None;
    for (model_name, field_mapping) in &data.temp_model_mappings.clone() {
        ui.horizontal(|ui| {
            ui.label("Model:");
            ui.strong(
                egui::RichText::new(model_name).color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.separator();
            ui.label("Term Field:");
            ui.monospace(
                egui::RichText::new(&field_mapping.term_field)
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.separator();
            ui.label("Reading Field:");
            ui.monospace(
                egui::RichText::new(&field_mapping.reading_field)
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );

            if ui.button("Edit").clicked() {
                *model_editor = ModelMappingEditor {
                    model_name: model_name.to_string(),
                    term_field: field_mapping.term_field.clone(),
                    reading_field: field_mapping.reading_field.clone(),
                    is_editing: true,
                    original_model_name: Some(model_name.to_string()),
                };
            }

            if ui.button("Delete").clicked() {
                to_remove = Some(model_name.clone());
            }
        });
    }

    if let Some(model_name) = to_remove {
        data.temp_model_mappings.remove(&model_name);
    }
}

fn show_mapping_editor_ui(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    model_editor: &mut ModelMappingEditor,
    available_models: &[AnkiModelInfo],
    anki_service: &mut AnkiService,
    data: &mut SettingsModalData,
) {
    model_selection_ui(ui, model_editor, available_models, anki_service, _ctx);

    if let Some(selected_model) =
        available_models.iter().find(|m| m.name == model_editor.model_name)
    {
        field_selection_ui(
            ui,
            "Term Field",
            &mut model_editor.term_field,
            &selected_model.fields,
            selected_model.sample_note.as_ref(),
            egui::Color32::from_rgb(100, 200, 100),
            "term_field_combo",
        );

        field_selection_ui(
            ui,
            "Reading Field",
            &mut model_editor.reading_field,
            &selected_model.fields,
            selected_model.sample_note.as_ref(),
            egui::Color32::from_rgb(100, 150, 255),
            "reading_field_combo",
        );
    }

    ui.horizontal(|ui| {
        let save_text = if model_editor.is_editing { "Update" } else { "Add" };

        if ui.button(save_text).clicked() {
            if !model_editor.model_name.is_empty()
                && !model_editor.term_field.is_empty()
                && !model_editor.reading_field.is_empty()
            {
                if let Some(original_name) = &model_editor.original_model_name {
                    if original_name != &model_editor.model_name {
                        data.temp_model_mappings.remove(original_name);
                    }
                }

                data.temp_model_mappings.insert(
                    model_editor.model_name.clone(),
                    FieldMapping {
                        term_field: model_editor.term_field.clone(),
                        reading_field: model_editor.reading_field.clone(),
                    },
                );

                *model_editor = ModelMappingEditor::default();
            }
        }
    });
}
