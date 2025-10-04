use std::sync::{
    Arc,
    Mutex,
};

use eframe::egui;
use egui_flex::{
    item,
    Flex,
};

use crate::core::{
    utils::text_matches_search,
    IgnoreList,
    DEFAULT_IGNORED_TERMS,
};

pub struct IgnoreListModal {
    open: bool,
    new_term: String,
    search_filter: String,
    temp_terms: Vec<String>,
    original_terms: Vec<String>,
}

impl IgnoreListModal {
    pub fn new() -> Self {
        Self {
            open: false,
            new_term: String::new(),
            search_filter: String::new(),
            temp_terms: Vec::new(),
            original_terms: Vec::new(),
        }
    }

    pub fn open_modal(&mut self, ignore_list: &Arc<Mutex<IgnoreList>>) {
        self.open = true;
        self.new_term.clear();
        self.search_filter.clear();

        if let Ok(list) = ignore_list.lock() {
            self.temp_terms = list.get_all_terms();
            self.original_terms = self.temp_terms.clone();
        }
    }

    fn is_dirty(&self) -> bool {
        self.temp_terms != self.original_terms
    }

    fn matches_search_term(&self, term: &str, search_query: &str) -> bool {
        text_matches_search(term, search_query)
    }

    pub fn show(&mut self, ctx: &egui::Context, ignore_list: &Arc<Mutex<IgnoreList>>) -> bool {
        if !self.open {
            return false;
        }

        let mut changed = false;

        let modal = egui::Modal::new(egui::Id::new("ignore_list_modal")).show(ctx, |ui| {
            ui.heading("Ignore List");
            ui.add_space(10.0);

            self.ui_controls(ui);
            ui.add_space(5.0);

            self.ui_list(ui);

            ui.separator();

            // Reserve space to prevent modal resizing
            ui.horizontal(|ui| {
                let is_dirty = self.is_dirty();
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
                let is_dirty = self.is_dirty();
                let save_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Save Settings")).clicked();
                let cancel_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Cancel")).clicked();

                let mut reset_clicked = false;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    reset_clicked = ui.button("Restore Default").clicked();
                });

                if save_clicked {
                    if let Ok(mut list) = ignore_list.lock() {
                        if let Err(e) = list.clear_all() {
                            eprintln!("Failed to clear ignore list: {}", e);
                        } else {
                            let mut save_success = true;
                            for term in &self.temp_terms {
                                if let Err(e) = list.add_term(term) {
                                    eprintln!(
                                        "Failed to add term '{}' to ignore list: {}",
                                        term, e
                                    );
                                    save_success = false;
                                }
                            }
                            if save_success {
                                self.original_terms = self.temp_terms.clone();
                                changed = true;
                            }
                        }
                    }
                    ui.close();
                } else if cancel_clicked {
                    self.temp_terms = self.original_terms.clone();
                } else if reset_clicked {
                    self.temp_terms = DEFAULT_IGNORED_TERMS.iter().map(|s| s.to_string()).collect();
                }
            });
        });

        if modal.should_close() {
            self.open = false;
        }

        changed
    }

    fn ui_controls(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Add New Term:");
                    ui.horizontal(|ui| {
                        let response = ui.text_edit_singleline(&mut self.new_term);

                        let add_clicked = ui.button("Add").clicked();
                        let enter_pressed =
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        if (add_clicked || enter_pressed) && !self.new_term.trim().is_empty() {
                            let term = self.new_term.trim().to_string();
                            // Validate term is not just whitespace and isn't already present
                            if !term.is_empty() && !self.temp_terms.contains(&term) {
                                self.temp_terms.push(term);
                                self.new_term.clear();
                            }
                        }
                    });
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.label("Search Terms:");
                    ui.text_edit_singleline(&mut self.search_filter);
                });
            });
        });
    }

    fn ui_list(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Current Terms:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Total: {}", self.temp_terms.len()));
                    });
                });

                ui.separator();

                egui::ScrollArea::vertical().auto_shrink([false; 2]).max_height(200.0).show(
                    ui,
                    |ui| {
                        let search_query = self.search_filter.trim();

                        let filtered_terms: Vec<_> = if search_query.is_empty() {
                            self.temp_terms.clone()
                        } else {
                            self.temp_terms
                                .iter()
                                .filter(|term| self.matches_search_term(term, search_query))
                                .cloned()
                                .collect()
                        };

                        if filtered_terms.is_empty() {
                            ui.label("No terms found");
                        } else {
                            let mut to_remove = None;

                            let terms_to_display = filtered_terms.clone();
                            Flex::horizontal().wrap(true).show(ui, |flex| {
                                for term in &terms_to_display {
                                    flex.add_ui(item(), |ui| {
                                        egui::Frame::new()
                                            .fill(ui.visuals().widgets.inactive.bg_fill)
                                            .stroke(ui.visuals().widgets.inactive.bg_stroke)
                                            .corner_radius(4.0)
                                            .inner_margin(6.0)
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(term).size(16.0));
                                                    ui.add_space(4.0);

                                                    let close_button = egui::Button::new(
                                                        egui::RichText::new("×")
                                                            .color(egui::Color32::RED),
                                                    )
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .stroke(egui::Stroke::NONE)
                                                    .small();

                                                    let response = ui.add(close_button);

                                                    if response.hovered() {
                                                        ui.ctx().set_cursor_icon(
                                                            egui::CursorIcon::PointingHand,
                                                        );
                                                    }

                                                    if response.clicked() {
                                                        if let Some(actual_index) = self
                                                            .temp_terms
                                                            .iter()
                                                            .position(|t| t == term)
                                                        {
                                                            to_remove = Some(actual_index);
                                                        }
                                                    }
                                                });
                                            });
                                    });
                                }
                            });

                            if let Some(index) = to_remove {
                                self.temp_terms.remove(index);
                            }
                        }
                    },
                );
            });
        });
    }
}
