use std::collections::HashMap;

use eframe::egui::{
    self,
    DragValue,
    Modal,
    Slider,
    SliderClamping,
};
use egui_extras::{
    Column,
    TableBuilder,
};

use super::{
    data::FrequencyDictionarySetting,
    SettingsData,
};
use crate::dictionary::frequency_manager::FrequencyManager;

#[derive(Clone, Debug, PartialEq)]
struct FrequencyEntry {
    name: String,
    weight: f32,
    enabled: bool,
}

pub struct FrequencyWeightsModal {
    open: bool,
    entries: Vec<FrequencyEntry>,
    original: Vec<FrequencyEntry>,
}

impl Default for FrequencyWeightsModal {
    fn default() -> Self {
        Self::new()
    }
}

impl FrequencyWeightsModal {
    pub fn new() -> Self {
        Self { open: false, entries: Vec::new(), original: Vec::new() }
    }

    pub fn open_modal(&mut self, settings: &SettingsData, manager: Option<&FrequencyManager>) {
        self.entries = build_entries(settings, manager);
        self.original = self.entries.clone();
        self.open = true;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn is_dirty(&self) -> bool {
        self.entries != self.original
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<HashMap<String, FrequencyDictionarySetting>> {
        if !self.open {
            return None;
        }

        let mut result: Option<HashMap<String, FrequencyDictionarySetting>> = None;

        let modal = Modal::new(egui::Id::new("frequency_weights_modal")).show(ctx, |ui| {
            ui.heading("Frequency Dictionary Weights");
            ui.add_space(6.0);

            if self.entries.is_empty() {
                ui.label("No frequency dictionaries loaded.");
            } else {
                egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(
                    ui,
                    |ui| {
                        TableBuilder::new(ui)
                            .striped(true)
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .column(Column::exact(26.0))
                            .column(Column::auto().at_least(170.0))
                            .column(Column::remainder().at_least(220.0))
                            .column(Column::exact(84.0))
                            .header(24.0, |mut header| {
                                header.col(|_ui| {});
                                header.col(|ui| {
                                    ui.label(egui::RichText::new("Dictionary").strong());
                                });
                                header.col(|ui| {
                                    ui.label(egui::RichText::new("Weight").strong());
                                });
                                header.col(|ui| {
                                    ui.label(egui::RichText::new("Value").strong());
                                });
                            })
                            .body(|body| {
                                body.rows(28.0, self.entries.len(), |mut row| {
                                    let idx = row.index();
                                    let entry = &mut self.entries[idx];

                                    row.col(|ui| {
                                        if ui
                                            .add(egui::widgets::Checkbox::without_text(
                                                &mut entry.enabled,
                                            ))
                                            .changed()
                                        {
                                            if !entry.enabled {
                                                entry.weight = entry.weight.max(0.1);
                                            }
                                        }
                                    });

                                    row.col(|ui| {
                                        let text_color = if entry.enabled {
                                            ui.visuals().strong_text_color()
                                        } else {
                                            ui.visuals().weak_text_color()
                                        };
                                        ui.label(
                                            egui::RichText::new(&entry.name).color(text_color),
                                        );
                                    });

                                    row.col(|ui| {
                                        let slider = Slider::new(&mut entry.weight, 0.1..=5.0)
                                            .logarithmic(true)
                                            .clamping(SliderClamping::Always)
                                            .show_value(false);
                                        ui.add_enabled_ui(entry.enabled, |ui| {
                                            let width = ui.available_width();
                                            ui.add_sized([width, 18.0], slider);
                                        });
                                    });

                                    row.col(|ui| {
                                        ui.add_enabled_ui(entry.enabled, |ui| {
                                            ui.add_sized(
                                                [70.0, 20.0],
                                                DragValue::new(&mut entry.weight)
                                                    .range(0.1..=5.0)
                                                    .speed(0.05)
                                                    .fixed_decimals(2)
                                                    .suffix("x"),
                                            );
                                        });
                                    });
                                });
                            });
                    },
                );
            }

            ui.separator();

            let is_dirty = self.is_dirty();

            ui.horizontal(|ui| {
                if is_dirty {
                    ui.colored_label(egui::Color32::YELLOW, "⚠");
                    ui.label("Settings have been modified");
                } else {
                    ui.colored_label(egui::Color32::TRANSPARENT, "⚠");
                    ui.label("");
                }
            });
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let save_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Save Settings")).clicked();
                let cancel_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Cancel")).clicked();

                let mut reset_clicked = false;
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    reset_clicked = ui.button("Restore Default").clicked();
                });

                if save_clicked {
                    result = Some(entries_to_map(&self.entries));
                    self.original = self.entries.clone();
                    ui.close();
                } else if cancel_clicked {
                    self.entries = self.original.clone();
                } else if reset_clicked {
                    for entry in &mut self.entries {
                        entry.enabled = true;
                        entry.weight = 1.0;
                    }
                }
            });
        });

        if modal.should_close() {
            self.open = false;
        }

        result
    }
}

fn build_entries(
    settings: &SettingsData,
    manager: Option<&FrequencyManager>,
) -> Vec<FrequencyEntry> {
    let mut entries = Vec::new();

    if let Some(manager) = manager {
        if let Some(states) = manager.dictionary_states() {
            for (name, state) in states {
                let settings_state = settings.frequency_weights.get(&name);
                entries.push(FrequencyEntry {
                    name,
                    weight: settings_state.map(|s| s.weight).unwrap_or(state.weight).max(0.1),
                    enabled: settings_state.map(|s| s.enabled).unwrap_or(state.enabled),
                });
            }
        }
    }

    if entries.is_empty() {
        for (name, setting) in &settings.frequency_weights {
            entries.push(FrequencyEntry {
                name: name.clone(),
                weight: setting.weight,
                enabled: setting.enabled,
            });
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn entries_to_map(entries: &[FrequencyEntry]) -> HashMap<String, FrequencyDictionarySetting> {
    entries
        .iter()
        .map(|entry| {
            (
                entry.name.clone(),
                FrequencyDictionarySetting { weight: entry.weight, enabled: entry.enabled },
            )
        })
        .collect()
}
