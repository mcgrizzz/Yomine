use std::collections::HashMap;

use eframe::egui::{self, RichText};

use super::{
    anki_service::AnkiService,
    components::{
        ui_connection_status,
        ui_field_selection,
        ui_model_selection,
    },
    data::{
        AnkiModelInfo,
        ModelMappingEditor,
        SettingsData,
    },
};
use crate::anki::FieldMapping;

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
            || self.settings.anki_interval != self.original_settings.anki_interval
    }
}

pub struct AnkiSettingsModal {
    open: bool,
    data: SettingsModalData,
    model_editor: ModelMappingEditor,
    available_models: Vec<AnkiModelInfo>,
    anki_service: AnkiService,
}

impl AnkiSettingsModal {
    pub fn new() -> Self {
        Self {
            open: false,
            data: SettingsModalData::default(),
            model_editor: ModelMappingEditor::default(),
            available_models: Vec::new(),
            anki_service: AnkiService::new(),
        }
    }

    pub fn open_settings(&mut self, current_settings: SettingsData, ctx: &egui::Context) {
        self.data.settings = current_settings.clone();
        self.data.temp_model_mappings = current_settings.anki_model_mappings.clone();
        self.data.original_settings = current_settings.clone();
        self.open = true;

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
        self.open
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<SettingsData> {
        if !self.open {
            return None;
        }

        self.anki_service.check_async_results(&mut self.available_models, ctx);

        let available_models = self.available_models.clone();
        let mut anki_service = std::mem::take(&mut self.anki_service);
        let mut model_editor = std::mem::take(&mut self.model_editor);

        let mut result: Option<SettingsData> = None;

        let modal = egui::Modal::new(egui::Id::new("anki_settings_modal")).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui_known_interval_setting(ui, &mut self.data);
                ui.separator();

                ui_existing_mappings(ui, &mut self.data, &mut model_editor);
                ui.separator();

                ui.heading(if model_editor.is_editing {
                    "Edit Model Mapping"
                } else {
                    "Add Model Mapping"
                });
                ui.add_space(1.0);
                ui_connection_status(ui, &mut anki_service, ctx);
                ui.add_space(5.0);

                ui_mapping_editor(
                    ui,
                    ctx,
                    &mut model_editor,
                    &available_models,
                    &mut anki_service,
                    &mut self.data,
                );
            });

            ui.separator();

            let is_dirty = self.data.is_dirty();
            if is_dirty {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::YELLOW, "⚠");
                    ui.label("Settings have been modified");
                });
                ui.add_space(5.0);
            }

            ui.horizontal(|ui| {
                let is_dirty = self.data.is_dirty();

                let save_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Save Settings")).clicked();
                let cancel_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Cancel")).clicked();

                let mut reset_clicked = false;
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    reset_clicked = ui.button("Restore Default").clicked();
                });

                if save_clicked {
                    let mut settings = self.data.settings.clone();
                    settings.anki_model_mappings = self.data.temp_model_mappings.clone();
                    self.data.original_settings = settings.clone();
                    result = Some(settings);
                    ui.close();
                } else if cancel_clicked {
                    self.data.temp_model_mappings =
                        self.data.original_settings.anki_model_mappings.clone();
                    self.data.settings = self.data.original_settings.clone();
                } else if reset_clicked {
                    self.data.temp_model_mappings.clear();
                    self.data.settings = SettingsData::new();
                }
            })
        });

        self.anki_service = anki_service;
        self.model_editor = model_editor;

        if modal.should_close() {
            self.open = false;
        }

        result
    }
}

impl Default for AnkiSettingsModal {
    fn default() -> Self {
        Self::new()
    }
}

fn ui_existing_mappings(
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

fn ui_mapping_editor(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    model_editor: &mut ModelMappingEditor,
    available_models: &[AnkiModelInfo],
    anki_service: &mut AnkiService,
    data: &mut SettingsModalData,
) {
    ui_model_selection(ui, model_editor, available_models, anki_service, _ctx);

    if let Some(selected_model) =
        available_models.iter().find(|m| m.name == model_editor.model_name)
    {
        ui_field_selection(
            ui,
            "Term Field",
            &mut model_editor.term_field,
            &selected_model.fields,
            selected_model.sample_note.as_ref(),
            egui::Color32::from_rgb(100, 200, 100),
            "term_field_combo",
        );

        ui_field_selection(
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

fn ui_known_interval_setting(ui: &mut egui::Ui, data: &mut SettingsModalData) {
    ui.horizontal(|ui| {
        ui.heading("Known Interval Threshold");
        ui.label(
            RichText::new("ℹ")
                .color(ui.visuals().weak_text_color())
                .size(12.0),
        )
        .on_hover_text(
            "Cards with an interval at or above this threshold will be considered 'known' \
             for comprehensibility estimation.",
        );
    });
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.label("Interval:");

        ui.add(
            egui::DragValue::new(&mut data.settings.anki_interval)
                .speed(1.0)
                .range(1..=365)
                .suffix(" days"),
        );

        ui.label("(Default: 30 days)");
    });

    ui.add_space(5.0);
}
