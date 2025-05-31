use eframe::egui;
use rfd::FileDialog;

use super::{
    modal::{
        Modal,
        ModalConfig,
        ModalResult,
    },
    recent_files::RecentFiles,
    theme::Theme,
};
use crate::{
    core::{
        filename_parser,
        models::FileType,
        SourceFile,
    },
    persistence::{
        load_json_or_default,
        save_json,
    },
};

pub struct FileModal {
    modal: Modal<SourceFile>,
    recent_files: RecentFiles,
}

impl FileModal {
    pub fn new() -> Self {
        let config = ModalConfig {
            fixed_size: Some(egui::Vec2::new(600.0, 400.0)),
            centered: true,
            show_overlay: true,
            ..Default::default()
        };

        let recent_files = load_json_or_default::<RecentFiles>("recent_files.json");

        Self {
            modal: Modal::new_with_data("Open File", SourceFile::default()).with_config(config),
            recent_files,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        current_file_path: Option<&str>,
    ) -> Option<SourceFile> {
        self.recent_files.cleanup_missing_files();

        let recent_files = &self.recent_files;
        let current_file_path_owned = current_file_path.map(|s| s.to_string());

        if let Some(result) = self.modal.show(ctx, |ui, _data| {
            let mut modal_result: Option<ModalResult<SourceFile>> = None;

            if let Some(result) = Self::browse_button(ui) {
                modal_result = Some(result);
            }

            if modal_result.is_none() {
                if let Some(result) = Self::render_recent_files_section(
                    ui,
                    theme,
                    recent_files,
                    current_file_path_owned.as_deref(),
                ) {
                    modal_result = Some(result);
                }
            }

            if modal_result.is_none() {
                modal_result = Self::render_cancel_button(ui);
            }

            modal_result
        }) {
            self.handle_modal_result(result)
        } else {
            None
        }
    }

    fn browse_button(ui: &mut egui::Ui) -> Option<ModalResult<SourceFile>> {
        ui.vertical_centered(|ui| {
            if ui
                .add_sized(
                    [200.0, 35.0],
                    egui::Button::new(egui::RichText::new("Browse for File").size(14.0)),
                )
                .clicked()
            {
                Self::handle_file_browse()
            } else {
                None
            }
        })
        .inner
    }

    fn handle_file_browse() -> Option<ModalResult<SourceFile>> {
        FileDialog::new()
            .add_filter("Subtitle files", &["srt", "vtt", "ass"])
            .add_filter("Text files", &["txt"])
            .pick_file()
            .map(|path| {
                let source_file = Self::create_source_file_from_path(&path);
                ModalResult::Confirmed(source_file)
            })
    }

    fn create_source_file_from_path(path: &std::path::Path) -> SourceFile {
        Self::create_source_file_from_path_and_metadata(path, None, None)
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
            id: 3,
            source: None,
            file_type: FileType::from_extension(&file_path_str),
            title,
            creator,
            original_file: file_path_str,
        }
    }

    fn render_recent_files_section(
        ui: &mut egui::Ui,
        theme: &Theme,
        recent_files: &RecentFiles,
        current_file_path: Option<&str>,
    ) -> Option<ModalResult<SourceFile>> {
        ui.add_space(8.0);

        if recent_files.is_empty() {
            Self::render_no_recent_files(ui, theme);
            return None;
        }

        ui.separator();
        ui.label(egui::RichText::new("Recent Files").color(theme.cyan(ui.ctx())).size(13.0));
        ui.add_space(3.0);

        let mut modal_result = None;
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for entry in recent_files.get_valid_files() {
                if let Some(result) = Self::recent_file(ui, entry, current_file_path, theme) {
                    modal_result = Some(result);
                    break;
                }
                ui.add_space(1.0);
            }
        });

        modal_result
    }

    fn render_no_recent_files(ui: &mut egui::Ui, theme: &Theme) {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No recent files").color(theme.comment(ui.ctx())).size(11.0),
            );
        });
    }

    fn recent_file(
        ui: &mut egui::Ui,
        entry: &crate::gui::recent_files::RecentFileEntry,
        current_file_path: Option<&str>,
        theme: &Theme,
    ) -> Option<ModalResult<SourceFile>> {
        let is_current_file =
            current_file_path.map(|current| current == entry.file_path).unwrap_or(false);

        ui.group(|ui| {
            ui.set_min_width(ui.available_width() - 20.0);

            if is_current_file {
                Self::apply_current_file_styling(ui, theme);
            }

            if let Some(result) = Self::file_button(ui, entry, is_current_file, theme) {
                return Some(result);
            }

            Self::render_file_info_row(ui, entry, is_current_file, theme);

            None
        })
        .inner
    }

    fn apply_current_file_styling(ui: &mut egui::Ui, theme: &Theme) {
        let current_bg = theme.highlight(ui.ctx());
        ui.style_mut().visuals.widgets.inactive.weak_bg_fill = current_bg;
        ui.style_mut().visuals.widgets.inactive.bg_fill = current_bg;
    }

    fn file_button(
        ui: &mut egui::Ui,
        entry: &crate::gui::recent_files::RecentFileEntry,
        is_current_file: bool,
        theme: &Theme,
    ) -> Option<ModalResult<SourceFile>> {
        ui.horizontal(|ui| {
            let button_text = if entry.title.trim().is_empty() {
                entry.get_filename()
            } else {
                entry.title.clone()
            };

            let button = Self::create_file_button(&button_text, is_current_file, theme, ui.ctx());
            let button_response = ui.add_enabled(!is_current_file, button);

            if button_response.clicked() && !is_current_file {
                return Some(Self::create_file_selection_result(entry));
            }

            Self::render_metadata_labels(ui, entry, is_current_file, theme);
            None
        })
        .inner
    }

    fn create_file_button(
        text: &str,
        is_current_file: bool,
        theme: &Theme,
        ctx: &egui::Context,
    ) -> egui::Button<'static> {
        let text_color = if is_current_file { theme.cyan(ctx) } else { theme.foreground(ctx) };

        let mut button =
            egui::Button::new(egui::RichText::new(text.to_string()).size(11.5).color(text_color));

        if is_current_file {
            button = button.fill(theme.background_fill(ctx));
        }

        button
    }

    fn create_file_selection_result(
        entry: &crate::gui::recent_files::RecentFileEntry,
    ) -> ModalResult<SourceFile> {
        let source_file = SourceFile {
            id: 3,
            source: None,
            file_type: FileType::from_extension(&entry.file_path),
            title: entry.title.clone(),
            creator: entry.creator.clone(),
            original_file: entry.file_path.clone(),
        };

        ModalResult::Confirmed(source_file)
    }

    fn render_metadata_labels(
        ui: &mut egui::Ui,
        entry: &crate::gui::recent_files::RecentFileEntry,
        is_current_file: bool,
        theme: &Theme,
    ) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let metadata_color = if is_current_file {
                theme.comment(ui.ctx()).linear_multiply(0.7) // Darker comment color for current file
            } else {
                theme.comment(ui.ctx())
            };

            let term_count_color = if is_current_file {
                theme.blue(ui.ctx()).linear_multiply(0.8) // Darker blue for current file
            } else {
                theme.blue(ui.ctx())
            };

            ui.label(
                egui::RichText::new(&entry.format_last_opened()).color(metadata_color).size(8.0),
            );
            ui.label(
                egui::RichText::new(&entry.format_term_count()).color(term_count_color).size(8.0),
            );
            ui.label(
                egui::RichText::new(&entry.format_file_size()).color(metadata_color).size(8.0),
            );
        });
    }

    fn render_file_info_row(
        ui: &mut egui::Ui,
        entry: &crate::gui::recent_files::RecentFileEntry,
        is_current_file: bool,
        theme: &Theme,
    ) {
        ui.horizontal(|ui| {
            ui.add_space(6.0);

            let info_color = if is_current_file {
                theme.comment(ui.ctx()).linear_multiply(0.6) // Darker comment color for current file
            } else {
                theme.comment(ui.ctx()).linear_multiply(0.9) // Slightly muted comment color
            };

            let creator_color = if is_current_file {
                theme.comment(ui.ctx()).linear_multiply(0.65) // Slightly different shade for current file
            } else {
                theme.comment(ui.ctx())
            };

            // Show filename if different from title
            if !entry.title.trim().is_empty() && entry.title != entry.get_filename() {
                ui.label(egui::RichText::new(&entry.get_filename()).color(info_color).size(9.0));
            }

            // Show creator if available
            if let Some(creator) = &entry.creator {
                ui.label(
                    egui::RichText::new(format!("• {}", creator)).color(creator_color).size(9.0),
                );
            }

            // Show "Currently Open" indicator
            if is_current_file {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("(Currently Open)")
                            .color(theme.cyan(ui.ctx()))
                            .size(10.0)
                            .italics(),
                    );
                });
            }
        });
    }

    fn render_cancel_button(ui: &mut egui::Ui) -> Option<ModalResult<SourceFile>> {
        ui.add_space(15.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                Some(ModalResult::Cancelled)
            } else {
                None
            }
        })
        .inner
    }

    fn handle_modal_result(&mut self, result: ModalResult<SourceFile>) -> Option<SourceFile> {
        match result {
            ModalResult::Confirmed(source_file) => {
                self.modal.close();
                *self.modal.data_mut() = SourceFile::default();
                Some(source_file)
            }
            ModalResult::Cancelled => {
                *self.modal.data_mut() = SourceFile::default();
                None
            }
            _ => None,
        }
    }

    pub fn open_dialog(&mut self) {
        self.modal.open();
    }

    pub fn close_dialog(&mut self) {
        self.modal.close();
    }

    pub fn is_open(&self) -> bool {
        self.modal.is_open()
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
}

impl Default for FileModal {
    fn default() -> Self {
        Self::new()
    }
}

impl FileType {
    fn from_extension(file_path: &str) -> Self {
        if let Some(extension) =
            std::path::Path::new(file_path).extension().and_then(|ext| ext.to_str())
        {
            match extension.to_lowercase().as_str() {
                "srt" => FileType::SRT,
                other => FileType::Other(other.to_uppercase()),
            }
        } else {
            FileType::SRT
        }
    }
}
