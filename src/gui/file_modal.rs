use eframe::egui;
use rfd::FileDialog;

use super::{
    recent_files::RecentFiles,
    theme::Theme,
};
use crate::{
    core::{
        filename_parser,
        models::SourceFileType,
        SourceFile,
    },
    persistence::{
        load_json_or_default,
        save_json,
    },
};

// Default ID for newly created source files
const DEFAULT_SOURCE_FILE_ID: u32 = 3;

pub struct FileModal {
    open: bool,
    recent_files: RecentFiles,
    selected_file: Option<SourceFile>,
}

impl FileModal {
    pub fn new() -> Self {
        let recent_files = load_json_or_default::<RecentFiles>("recent_files.json");

        Self { open: false, recent_files, selected_file: None }
    }

    pub fn open_dialog(&mut self) {
        self.open = true;
        self.selected_file = None;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        current_file_path: Option<&str>,
    ) -> Option<SourceFile> {
        if !self.open {
            return None;
        }

        self.recent_files.cleanup_missing_files();
        let mut result: Option<SourceFile> = None;

        let modal = egui::Modal::new(egui::Id::new("file_modal")).show(ctx, |ui| {
            // Get the actual window size, not just available UI space
            let window_rect = ctx.input(|i| i.screen_rect());
            let modal_width = Self::calculate_modal_width(window_rect.width());

            ui.set_width(modal_width);
            ui.set_max_height(500.0);

            ui.heading("Open File");
            ui.add_space(10.0);

            if self.ui_browse_button(ui) {
                ui.close();
            }

            ui.add_space(15.0);

            if let Some(file) = self.ui_recent_files(ui, theme, current_file_path) {
                self.selected_file = Some(file);
                ui.close();
            }

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                });
            });
        });

        if modal.should_close() {
            self.open = false;
            result = self.selected_file.take();
        }

        result
    }

    fn ui_browse_button(&mut self, ui: &mut egui::Ui) -> bool {
        ui.vertical_centered(|ui| {
            let button_width = (ui.available_width() * 0.6).clamp(150.0, 250.0);

            if ui.add_sized([button_width, 35.0], egui::Button::new("Browse for File")).clicked() {
                if let Some(file) = self.handle_file_browse() {
                    self.selected_file = Some(file);
                    return true;
                }
            }
            false
        })
        .inner
    }

    fn ui_recent_files(
        &mut self,
        ui: &mut egui::Ui,
        theme: &Theme,
        current_file_path: Option<&str>,
    ) -> Option<SourceFile> {
        ui.add_space(8.0);

        if self.recent_files.is_empty() {
            Self::ui_no_recent_files(ui, theme);
            return None;
        }

        ui.separator();
        let recent_count = self.recent_files.get_valid_files().len();
        ui.label(
            egui::RichText::new(format!("Recent Files ({})", recent_count))
                .color(theme.cyan(ui.ctx()))
                .size(13.0),
        );
        ui.add_space(3.0);

        let mut modal_result = None;
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for entry in self.recent_files.get_valid_files() {
                if let Some(result) = Self::ui_recent_file(ui, entry, current_file_path, theme) {
                    modal_result = Some(result);
                    break;
                }
                ui.add_space(1.0);
            }
        });

        modal_result
    }

    fn ui_no_recent_files(ui: &mut egui::Ui, theme: &Theme) {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No recent files").color(theme.comment(ui.ctx())).size(11.0),
            );
        });
    }

    fn ui_recent_file(
        ui: &mut egui::Ui,
        entry: &crate::gui::recent_files::RecentFileEntry,
        current_file_path: Option<&str>,
        theme: &Theme,
    ) -> Option<SourceFile> {
        let is_current_file =
            current_file_path.map(|current| current == entry.file_path).unwrap_or(false);

        ui.group(|ui| {
            ui.set_min_width(ui.available_width() - 20.0);

            if is_current_file {
                Self::ui_current_file_styling(ui, theme);
            }

            Self::ui_file_button(ui, entry, is_current_file, theme)
        })
        .inner
    }

    fn ui_file_button(
        ui: &mut egui::Ui,
        entry: &crate::gui::recent_files::RecentFileEntry,
        is_current_file: bool,
        theme: &Theme,
    ) -> Option<SourceFile> {
        ui.vertical(|ui| {
            let button_text = if entry.title.trim().is_empty() {
                entry.get_filename()
            } else {
                entry.title.clone()
            };

            let text_color =
                if is_current_file { theme.cyan(ui.ctx()) } else { theme.foreground(ui.ctx()) };

            // Button takes full width
            let button_response = ui.add_sized(
                [ui.available_width(), 25.0],
                egui::Button::new(egui::RichText::new(&button_text).size(11.5).color(text_color))
                    .fill(if is_current_file {
                        theme.background_fill(ui.ctx())
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    })
                    .wrap(),
            );

            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width() * 0.6, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let info_color = if is_current_file {
                            theme.comment(ui.ctx()).linear_multiply(0.6)
                        } else {
                            theme.comment(ui.ctx()).linear_multiply(0.9)
                        };

                        if !entry.title.trim().is_empty() && entry.title != entry.get_filename() {
                            ui.label(
                                egui::RichText::new(&entry.get_filename())
                                    .color(info_color)
                                    .size(9.0),
                            );
                        }
                    },
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let metadata_color = if is_current_file {
                        theme.comment(ui.ctx()).linear_multiply(0.7)
                    } else {
                        theme.comment(ui.ctx())
                    };

                    let term_count_color = if is_current_file {
                        theme.blue(ui.ctx()).linear_multiply(0.8)
                    } else {
                        theme.blue(ui.ctx())
                    };

                    let creator_color = if is_current_file {
                        theme.orange(ui.ctx()).linear_multiply(0.65)
                    } else {
                        theme.orange(ui.ctx())
                    };

                    ui.label(
                        egui::RichText::new(&entry.format_term_count())
                            .color(term_count_color)
                            .size(8.0),
                    );

                    if let Some(creator) = &entry.creator {
                        ui.label(
                            egui::RichText::new(format!("📷 {}", creator))
                                .color(creator_color)
                                .size(8.0),
                        );
                    }

                    ui.label(
                        egui::RichText::new(&entry.format_last_opened())
                            .color(metadata_color)
                            .size(8.0),
                    );

                    ui.label(
                        egui::RichText::new(&entry.format_file_size())
                            .color(metadata_color)
                            .size(8.0),
                    );
                });
            });

            if button_response.clicked() && !is_current_file {
                return Some(Self::create_file_selection_result(entry));
            }

            None
        })
        .inner
    }

    fn ui_current_file_styling(ui: &mut egui::Ui, theme: &Theme) {
        let current_bg = theme.highlight(ui.ctx());
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = current_bg;
        ui.style_mut().visuals.widgets.inactive.bg_fill = current_bg;
    }

    fn handle_file_browse(&mut self) -> Option<SourceFile> {
        FileDialog::new()
            .add_filter("Subtitle files", &["srt", "vtt", "ass"])
            .add_filter("Text files", &["txt"])
            .pick_file()
            .map(|path| Self::create_source_file_from_path_and_metadata(&path, None, None))
    }

    fn create_source_file_from_path_and_metadata(
        path: &std::path::Path,
        title_override: Option<String>,
        creator_override: Option<String>,
    ) -> SourceFile {
        let filename = path.file_name().and_then(|name| name.to_str()).unwrap_or("Unknown");
        let file_path_str = path.display().to_string();

        let (title, creator) =
            if let (Some(title), Some(creator)) = (&title_override, &creator_override) {
                (title.clone(), Some(creator.clone()))
            } else {
                let media_info = filename_parser::parse_filename(filename);
                let parsed_title = media_info.display_title();
                let metadata_string = media_info.get_metadata_string();
                (
                    title_override.unwrap_or(parsed_title),
                    creator_override.or_else(|| {
                        if !metadata_string.is_empty() {
                            Some(metadata_string)
                        } else {
                            None
                        }
                    }),
                )
            };

        SourceFile {
            id: DEFAULT_SOURCE_FILE_ID,
            source: None,
            file_type: SourceFileType::from_extension(&file_path_str),
            title,
            creator,
            original_file: file_path_str,
        }
    }

    fn create_file_selection_result(
        entry: &crate::gui::recent_files::RecentFileEntry,
    ) -> SourceFile {
        SourceFile {
            id: DEFAULT_SOURCE_FILE_ID,
            source: None,
            file_type: SourceFileType::from_extension(&entry.file_path),
            title: entry.title.clone(),
            creator: entry.creator.clone(),
            original_file: entry.file_path.clone(),
        }
    }

    pub fn add_recent_file(
        &mut self,
        file_path: String,
        title: String,
        creator: Option<String>,
        term_count: usize,
    ) {
        self.recent_files.add_file(file_path, title, creator, term_count);
        self.save_recent_files();
    }

    pub fn save_recent_files(&self) {
        if let Err(e) = save_json(&self.recent_files, "recent_files.json") {
            eprintln!("Failed to save recent files: {}", e);
        }
    }

    fn calculate_modal_width(window_width: f32) -> f32 {
        let min_width = 400.0;
        let max_width = 800.0;
        let min_percent = 0.65;
        let max_percent = 1.00;

        let percent = {
            let t = (window_width - min_width) / (max_width - min_width);
            let interpolated = max_percent + (min_percent - max_percent) * t;
            interpolated.max(min_percent).min(max_percent)
        };
        max_width.min(window_width * percent)
    }
}

impl Default for FileModal {
    fn default() -> Self {
        Self::new()
    }
}
