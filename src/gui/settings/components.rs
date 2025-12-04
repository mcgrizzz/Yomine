use eframe::egui;
use wana_kana::{
    utils::{
        is_char_kana,
        is_char_kanji,
    },
    IsJapaneseChar,
};

use super::{
    anki_service::AnkiService,
    data::{
        AnkiModelInfo,
        ModelMappingEditor,
    },
};

pub fn ui_connection_status(
    ui: &mut egui::Ui,
    anki_service: &mut AnkiService,
    ctx: &egui::Context,
) {
    ui.horizontal(|ui| {
        ui.label("Anki Connection Status:");

        if anki_service.is_loading_models {
            ui.spinner();
        }

        ui.colored_label(
            if anki_service.connection_status.contains("Connected") {
                egui::Color32::GREEN
            } else if anki_service.connection_status.contains("Error") {
                egui::Color32::RED
            } else {
                egui::Color32::YELLOW
            },
            &anki_service.connection_status,
        );

        if ui
            .add_enabled(
                !anki_service.is_loading_models,
                egui::Button::new(if anki_service.is_loading_models {
                    "Refreshing..."
                } else {
                    "Refresh Notetypes"
                }),
            )
            .clicked()
            && !anki_service.is_loading_models
        {
            anki_service.fetch_models(ctx);
        }
    });
}

pub fn ui_model_selection(
    ui: &mut egui::Ui,
    model_editor: &mut ModelMappingEditor,
    available_models: &[AnkiModelInfo],
    anki_service: &mut AnkiService,
    ctx: &egui::Context,
) {
    ui.horizontal(|ui| {
        ui.label("Notetype:");
        let previous_model = model_editor.model_name.clone();
        egui::ComboBox::from_id_salt("model_name_combo")
            .selected_text(&model_editor.model_name)
            .show_ui(ui, |ui| {
                for model in available_models {
                    if ui
                        .selectable_value(
                            &mut model_editor.model_name,
                            model.name.clone(),
                            &model.name,
                        )
                        .clicked()
                    {
                        model_editor.term_field.clear();
                        model_editor.reading_field.clear();
                    }
                }
            });

        if !model_editor.model_name.is_empty()
            && model_editor.model_name != previous_model
            && !anki_service.is_loading_models
        {
            let has_sample = available_models
                .iter()
                .find(|m| m.name == model_editor.model_name)
                .map(|m| m.sample_note.is_some())
                .unwrap_or(false);

            if !has_sample {
                anki_service.fetch_sample_note(&model_editor.model_name.clone(), ctx);
            } else {
                trigger_field_guessing(model_editor, available_models);
            }
        }
    });

    if !model_editor.model_name.is_empty()
        && model_editor.term_field.is_empty()
        && model_editor.reading_field.is_empty()
    {
        if let Some(model) = available_models.iter().find(|m| m.name == model_editor.model_name) {
            if model.sample_note.is_some() {
                trigger_field_guessing(model_editor, available_models);
            }
        }
    }
}

fn trigger_field_guessing(
    model_editor: &mut ModelMappingEditor,
    available_models: &[AnkiModelInfo],
) {
    if let Some(model) = available_models.iter().find(|m| m.name == model_editor.model_name) {
        if let Some(sample_note) = &model.sample_note {
            let (best_term, best_reading) = guess_field_mappings(sample_note, &model.fields);

            if let Some(term_field) = best_term {
                model_editor.term_field = term_field;
            }

            if let Some(reading_field) = best_reading {
                model_editor.reading_field = reading_field;
            }
        }
    }
}

pub fn ui_field_selection(
    ui: &mut egui::Ui,
    label: &str,
    field_value: &mut String,
    available_fields: &[String],
    sample_note: Option<&std::collections::HashMap<String, String>>,
    color: egui::Color32,
    combo_id: &str,
) {
    ui.horizontal(|ui| {
        ui.label(format!("{}:", label));

        let is_auto_suggested = !field_value.is_empty() && sample_note.is_some();
        if is_auto_suggested {
            ui.label(egui::RichText::new("＊").color(egui::Color32::from_rgb(100, 200, 255)))
                .on_hover_text("This field was guessed based on its content");
        }

        egui::ComboBox::from_id_salt(combo_id).selected_text(field_value.as_str()).show_ui(
            ui,
            |ui| {
                for field in available_fields {
                    ui.selectable_value(field_value, field.clone(), field);
                }
            },
        );

        if !field_value.is_empty() {
            if let Some(sample_note) = sample_note {
                if let Some(example_value) = sample_note.get(field_value) {
                    ui.separator();
                    ui.label("Example:");
                    let display_value = if example_value.chars().count() > 30 {
                        let truncated: String = example_value.chars().take(27).collect();
                        format!("{}...", truncated)
                    } else {
                        example_value.clone()
                    };
                    ui.colored_label(color, format!("\"{}\"", display_value));
                }
            }
        }
    });
}

pub fn guess_field_mappings(
    sample_note: &std::collections::HashMap<String, String>,
    available_fields: &[String],
) -> (Option<String>, Option<String>) {
    let mut best_term = None;
    let mut best_reading = None;
    let mut term_index = None;

    for (field_index, field_name) in available_fields.iter().enumerate() {
        if let Some(field_value) = sample_note.get(field_name) {
            let trimmed_value = field_value.trim();
            if trimmed_value.is_empty() {
                continue;
            }

            if is_likely_term(trimmed_value) {
                if trimmed_value.chars().any(|c| is_char_kanji(c)) {
                    best_term = Some(field_name.clone());
                    term_index = Some(field_index);
                    break;
                } else if best_term.is_none() {
                    best_term = Some(field_name.clone());
                    term_index = Some(field_index);
                }
            }
        }
    }

    for (field_index, field_name) in available_fields.iter().enumerate() {
        if let Some(field_value) = sample_note.get(field_name) {
            let trimmed_value = field_value.trim();
            if trimmed_value.is_empty() {
                continue;
            }

            if is_likely_reading(trimmed_value) {
                if let Some(term_idx) = term_index {
                    if field_index > term_idx {
                        best_reading = Some(field_name.clone());
                        break;
                    } else if best_reading.is_none() {
                        best_reading = Some(field_name.clone());
                    }
                } else {
                    best_reading = Some(field_name.clone());
                    break;
                }
            }
        }
    }

    (best_term, best_reading)
}

fn is_likely_reading(value: &str) -> bool {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return false;
    }

    trimmed.chars().all(|c| is_char_kana(c) || c.is_whitespace())
}

fn is_likely_term(value: &str) -> bool {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return false;
    }

    trimmed.chars().all(|c| c.is_japanese() || c.is_whitespace())
}
