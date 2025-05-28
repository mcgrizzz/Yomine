use eframe::egui;
use rfd::FileDialog;

use super::modal::{
    Modal,
    ModalConfig,
    ModalResult,
};
use crate::core::{
    filename_parser,
    models::FileType,
    SourceFile,
};

#[derive(Default, Clone)]
pub struct FileData {
    pub file_path: String,
}

pub struct FileModal {
    modal: Modal<FileData>,
}

impl FileModal {
    pub fn new() -> Self {
        let config = ModalConfig {
            fixed_size: Some(egui::Vec2::new(500.0, 200.0)),
            centered: true,
            show_overlay: true,
            ..Default::default()
        };

        Self { modal: Modal::new_with_data("Open File", FileData::default()).with_config(config) }
    }
    pub fn show(&mut self, ctx: &egui::Context) -> Option<SourceFile> {
        if let Some(result) = self.modal.show(ctx, |ui, data| {
            ui.label("Select a file to open:");
            ui.add_space(10.0);

            if ui.button("Browse for File").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("Subtitle files", &["srt", "vtt", "ass"])
                    .add_filter("Text files", &["txt"])
                    .pick_file()
                {
                    data.file_path = path.display().to_string();
                }
            }
            if !data.file_path.is_empty() {
                ui.add_space(10.0);
                ui.label(format!(
                    "Selected: {}",
                    std::path::Path::new(&data.file_path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                ));

                ui.add_space(5.0);
                let filename = std::path::Path::new(&data.file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Unknown");

                let media_info = filename_parser::parse_filename(filename);
                let title = media_info.display_title();
                let metadata = media_info.get_metadata_string();

                ui.label(
                    egui::RichText::new(format!("Title: {}", title))
                        .color(egui::Color32::from_rgb(100, 150, 255)),
                );

                if !metadata.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("Info: {}", metadata))
                            .color(egui::Color32::from_rgb(150, 150, 150))
                            .size(11.0),
                    );
                }
            }

            ui.add_space(15.0);

            let can_confirm = !data.file_path.is_empty();
            ui.horizontal(|ui| {
                if can_confirm && ui.button("Confirm").clicked() {
                    Some(ModalResult::Confirmed(data.clone()))
                } else if ui.button("Cancel").clicked() {
                    Some(ModalResult::Cancelled)
                } else {
                    None
                }
            })
            .inner
        }) {
            match result {
                ModalResult::Confirmed(data) => {
                    let filename = std::path::Path::new(&data.file_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Unknown");

                    let media_info = filename_parser::parse_filename(filename);
                    let title = media_info.display_title();
                    let metadata_string = media_info.get_metadata_string();

                    println!("Filename: {}", filename);
                    println!("Parsed media: {:?}", media_info);
                    println!("Generated title: {}", title);
                    if !metadata_string.is_empty() {
                        println!("Extracted metadata: {}", metadata_string);
                    }

                    let source_file = SourceFile {
                        id: 3,
                        source: "USER_SELECTED".to_string(),
                        file_type: FileType::from_extension(&data.file_path),
                        title,
                        creator: if !metadata_string.is_empty() {
                            Some(metadata_string)
                        } else {
                            None
                        },
                        original_file: data.file_path,
                    };

                    self.modal.close();
                    *self.modal.data_mut() = FileData::default();

                    return Some(source_file);
                }
                ModalResult::Cancelled => {
                    *self.modal.data_mut() = FileData::default();
                }
                ModalResult::Custom(_, _) => {}
            }
        }

        None
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
