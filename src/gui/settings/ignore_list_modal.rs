use std::{
    collections::HashMap,
    sync::{
        Arc,
        Mutex,
    },
};

use eframe::egui;
use egui_flex::{
    item,
    Flex,
};

use crate::core::{
    ignore_list::IgnoreFile,
    utils::text_matches_search,
    IgnoreList,
    DEFAULT_IGNORED_TERMS,
};

enum FileAction {
    Toggle(usize, bool),
    Remove(usize),
    Refresh(usize),
    Add,
}

pub struct IgnoreListModal {
    open: bool,
    new_term: String,
    search_filter: String,
    temp_terms: Vec<String>,
    original_terms: Vec<String>,
    temp_files: Vec<IgnoreFile>,
    original_files: Vec<IgnoreFile>,
    file_term_counts: HashMap<String, usize>,
    export_error: Option<String>,
    export_success: bool,
}

impl Default for IgnoreListModal {
    fn default() -> Self {
        Self::new()
    }
}

impl IgnoreListModal {
    pub fn new() -> Self {
        Self {
            open: false,
            new_term: String::new(),
            search_filter: String::new(),
            temp_terms: Vec::new(),
            original_terms: Vec::new(),
            temp_files: Vec::new(),
            original_files: Vec::new(),
            file_term_counts: HashMap::new(),
            export_error: None,
            export_success: false,
        }
    }

    pub fn open_modal(&mut self, ignore_list: &Arc<Mutex<IgnoreList>>) {
        self.open = true;
        self.new_term.clear();
        self.search_filter.clear();
        self.export_error = None;
        self.export_success = false;

        if let Ok(list) = ignore_list.lock() {
            self.temp_terms = list.get_all_terms();
            self.original_terms = self.temp_terms.clone();
            self.temp_files = list.get_files();
            self.original_files = self.temp_files.clone();
            self.update_file_term_counts();
        }
    }

    fn update_file_term_counts(&mut self) {
        self.file_term_counts.clear();
        for file in &self.temp_files {
            if let Ok(terms) = IgnoreList::load_terms_from_file(&file.path) {
                self.file_term_counts.insert(file.path.clone(), terms.len());
            }
        }
    }

    fn is_dirty(&self) -> bool {
        self.temp_terms != self.original_terms || self.temp_files != self.original_files
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

            let file_to_refresh = self.ui_list(ui);

            // Handle file refresh - reload file cache in the actual ignore list
            if let Some(idx) = file_to_refresh {
                if let Ok(mut list) = ignore_list.lock() {
                    list.reload_file_cache();
                    changed = true;
                }
                // Update cached count for refreshed file
                if let Some(file) = self.temp_files.get(idx) {
                    if let Ok(terms) = IgnoreList::load_terms_from_file(&file.path) {
                        self.file_term_counts.insert(file.path.clone(), terms.len());
                    }
                }
            }

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

            if let Some(error_msg) = &self.export_error {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚠").color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                    ui.label(
                        egui::RichText::new(format!("Export failed: {}", error_msg))
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                });
                ui.add_space(8.0);
            } else if self.export_success {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("✓").color(egui::Color32::from_rgb(100, 200, 100)),
                    );
                    ui.label(
                        egui::RichText::new("Terms exported successfully")
                            .color(egui::Color32::from_rgb(100, 200, 100)),
                    );
                });
                ui.add_space(8.0);
            }

            ui.horizontal(|ui| {
                let is_dirty = self.is_dirty();
                let save_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Save Settings")).clicked();
                let cancel_clicked =
                    ui.add_enabled(is_dirty, egui::Button::new("Cancel")).clicked();

                let export_clicked = ui.button("Export...").clicked();

                let mut reset_clicked = false;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    reset_clicked = ui.button("Restore Default").clicked();
                });

                if save_clicked {
                    if let Ok(mut list) = ignore_list.lock() {
                        let mut save_success = true;

                        if let Err(e) = list.set_terms(self.temp_terms.clone()) {
                            eprintln!("Failed to save terms: {}", e);
                            save_success = false;
                        }

                        if let Err(e) = list.set_files(self.temp_files.clone()) {
                            eprintln!("Failed to save files: {}", e);
                            save_success = false;
                        }

                        if save_success {
                            self.original_terms = self.temp_terms.clone();
                            self.original_files = self.temp_files.clone();
                            changed = true;
                        }
                    }
                    ui.close();
                } else if cancel_clicked {
                    self.temp_terms = self.original_terms.clone();
                    self.temp_files = self.original_files.clone();
                } else if reset_clicked {
                    self.temp_terms = DEFAULT_IGNORED_TERMS.iter().map(|s| s.to_string()).collect();
                    self.temp_files.clear();
                }

                if export_clicked {
                    match self.export_terms() {
                        Ok(()) => {
                            self.export_error = None;
                            self.export_success = true;
                        }
                        Err(error_msg) => {
                            self.export_error = Some(error_msg);
                            self.export_success = false;
                        }
                    }
                }
            });
        });

        if modal.should_close() {
            self.open = false;
        }

        changed
    }

    fn render_file_pills(
        &self,
        ui: &mut egui::Ui,
        files_to_display: &[(usize, &IgnoreFile)],
        search_query: &str,
    ) -> Option<FileAction> {
        let mut action = None;

        Flex::horizontal().wrap(true).show(ui, |flex| {
            // Show file pills
            for (idx, file) in files_to_display {
                flex.add_ui(item(), |ui| {
                    let file_exists = IgnoreList::file_exists(&file.path);
                    let file_name = std::path::Path::new(&file.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&file.path);

                    let bg_color = if !file_exists {
                        ui.visuals().widgets.inactive.bg_fill.linear_multiply(0.5)
                    } else if file.enabled {
                        ui.visuals().widgets.active.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };

                    egui::Frame::new()
                        .fill(bg_color)
                        .stroke(ui.visuals().widgets.inactive.bg_stroke)
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Checkbox
                                let mut enabled = file.enabled;
                                ui.style_mut().visuals.widgets.inactive.bg_stroke =
                                    egui::Stroke::new(1.0, ui.visuals().text_color());
                                if ui.checkbox(&mut enabled, "").changed() {
                                    action = Some(FileAction::Toggle(*idx, enabled));
                                }

                                ui.label("📄");
                                let text_color = if !file_exists {
                                    ui.visuals().weak_text_color()
                                } else {
                                    ui.visuals().text_color()
                                };
                                ui.colored_label(
                                    text_color,
                                    egui::RichText::new(file_name).size(12.0).strong(),
                                );

                                if !file_exists {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(200, 100, 100),
                                        "(missing)",
                                    );
                                }

                                ui.add_space(4.0);

                                // Refresh button
                                let refresh_button =
                                    egui::Button::new(egui::RichText::new("↻").size(14.0))
                                        .fill(egui::Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::NONE)
                                        .small();

                                let refresh_response = ui.add(refresh_button);
                                if refresh_response.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if refresh_response.clicked() {
                                    action = Some(FileAction::Refresh(*idx));
                                }

                                // Close button
                                let close_button = egui::Button::new(
                                    egui::RichText::new("×").color(egui::Color32::RED),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .small();

                                let response = ui.add(close_button);
                                if response.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if response.clicked() {
                                    action = Some(FileAction::Remove(*idx));
                                }
                            });
                        })
                        .response
                        .on_hover_text(&file.path);
                });
            }

            // Add "+" pill
            if search_query.is_empty() {
                flex.add_ui(item(), |ui| {
                    let response = egui::Frame::new()
                        .fill(ui.visuals().widgets.inactive.bg_fill)
                        .stroke(ui.visuals().widgets.inactive.bg_stroke)
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("+ Import File").size(12.0));
                        })
                        .response;

                    if response.interact(egui::Sense::click()).clicked() {
                        action = Some(FileAction::Add);
                    }

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
            }
        });

        action
    }

    fn render_term_pills(&self, ui: &mut egui::Ui, terms: &[String]) -> Option<usize> {
        let mut remove_idx = None;

        Flex::horizontal().wrap(true).show(ui, |flex| {
            for term in terms {
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
                                    egui::RichText::new("×").color(egui::Color32::RED),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .small();

                                let response = ui.add(close_button);

                                if response.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                if response.clicked() {
                                    if let Some(actual_index) =
                                        self.temp_terms.iter().position(|t| t == term)
                                    {
                                        remove_idx = Some(actual_index);
                                    }
                                }
                            });
                        });
                });
            }
        });

        remove_idx
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

    fn ui_list(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        let file_to_refresh_result = ui
            .group(|ui| {
                ui.vertical(|ui| {
                    // Header with counts
                    ui.horizontal(|ui| {
                        ui.label("Current Terms:");
                        let file_term_count: usize = self
                            .temp_files
                            .iter()
                            .filter(|f| f.enabled)
                            .filter_map(|f| self.file_term_counts.get(&f.path).copied())
                            .sum();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!(
                                "Manual: {} | From Files: {}",
                                self.temp_terms.len(),
                                file_term_count
                            ));
                        });
                    });

                    ui.separator();

                    let scroll_result = egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .max_height(200.0)
                        .show(ui, |ui| {
                            let search_query = self.search_filter.trim();
                            let mut file_to_refresh = None;

                            // Filter and render file pills
                            let files_to_display: Vec<(usize, &IgnoreFile)> = self
                                .temp_files
                                .iter()
                                .enumerate()
                                .filter(|(_, file)| {
                                    if search_query.is_empty() {
                                        true
                                    } else if let Ok(terms) =
                                        IgnoreList::load_terms_from_file(&file.path)
                                    {
                                        terms.iter().any(|term| {
                                            self.matches_search_term(term, search_query)
                                        })
                                    } else {
                                        false
                                    }
                                })
                                .collect();

                            // Render file pills and handle actions
                            if !files_to_display.is_empty() || search_query.is_empty() {
                                if let Some(action) =
                                    self.render_file_pills(ui, &files_to_display, search_query)
                                {
                                    match action {
                                        FileAction::Toggle(idx, enabled) => {
                                            self.temp_files[idx].enabled = enabled;
                                        }
                                        FileAction::Remove(idx) => {
                                            let removed_path = self.temp_files[idx].path.clone();
                                            self.temp_files.remove(idx);
                                            self.file_term_counts.remove(&removed_path);
                                        }
                                        FileAction::Refresh(idx) => {
                                            file_to_refresh = Some(idx);
                                        }
                                        FileAction::Add => {
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter("Text files", &["txt"])
                                                .pick_file()
                                            {
                                                let path_string =
                                                    path.to_string_lossy().to_string();
                                                if !self
                                                    .temp_files
                                                    .iter()
                                                    .any(|f| f.path == path_string)
                                                {
                                                    if let Ok(terms) =
                                                        IgnoreList::load_terms_from_file(
                                                            &path_string,
                                                        )
                                                    {
                                                        self.file_term_counts.insert(
                                                            path_string.clone(),
                                                            terms.len(),
                                                        );
                                                    }
                                                    self.temp_files.push(IgnoreFile {
                                                        path: path_string,
                                                        enabled: true,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Filter and render term pills
                            let filtered_terms: Vec<_> = if search_query.is_empty() {
                                self.temp_terms.clone()
                            } else {
                                self.temp_terms
                                    .iter()
                                    .filter(|term| self.matches_search_term(term, search_query))
                                    .cloned()
                                    .collect()
                            };

                            if !self.temp_files.is_empty() && !filtered_terms.is_empty() {
                                ui.separator();
                            }

                            if filtered_terms.is_empty() && self.temp_files.is_empty() {
                                ui.label("No terms found");
                            } else if !filtered_terms.is_empty() {
                                if let Some(idx) = self.render_term_pills(ui, &filtered_terms) {
                                    self.temp_terms.remove(idx);
                                }
                            }

                            file_to_refresh
                        });

                    scroll_result.inner
                })
                .inner
            })
            .inner;

        file_to_refresh_result
    }

    fn export_terms(&self) -> Result<(), String> {
        let date = chrono::Local::now().format("%Y-%m-%d");
        let default_filename = format!("yomine_ignored_terms_{}.txt", date);

        let path = rfd::FileDialog::new()
            .add_filter("Text files", &["txt"])
            .set_file_name(&default_filename)
            .save_file()
            .ok_or_else(|| "Export cancelled".to_string())?;

        let content = self.temp_terms.join("\n");
        std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(())
    }
}
