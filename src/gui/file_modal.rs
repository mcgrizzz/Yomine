use eframe::egui::{ self, Button, TextEdit };
use rfd::FileDialog;
use crate::core::{ models::FileType, SourceFile };

#[derive(Default)]
pub struct FileModal {
    pub open: bool,
    pub file_title: String,
    pub file_creator: String,
    pub file_path: String,
    pub source_file: Option<SourceFile>,
}

impl FileModal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.open {
            open_new_file_dialog(ctx, self);
        }
    }

    pub fn open_dialog(&mut self) {
        self.open = true;
    }

    pub fn close_dialog(&mut self) {
        self.open = false;
    }

    pub fn take_source_file(&mut self) -> Option<SourceFile> {
        self.source_file.take()
    }
}

fn open_new_file_dialog(ctx: &egui::Context, file_modal: &mut FileModal) {
    egui::Window
        ::new("Open New File")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Enter Title:");
            ui.add(
                TextEdit::singleline(&mut file_modal.file_title).hint_text(
                    "E.g., Dan Da Dan - S01E08"
                )
            );
            file_modal.file_title = "TEST".to_string();

            ui.label("Enter Creator (optional):");
            ui.add(TextEdit::singleline(&mut file_modal.file_creator).hint_text("E.g., Netflix"));
            file_modal.file_creator = "Netflix".to_string();

            if ui.button("Browse for File").clicked() {
                if let Some(path) = FileDialog::new().pick_file() {
                    file_modal.file_path = path.display().to_string();
                }
            }

            if !file_modal.file_path.is_empty() {
                ui.label(format!("Selected File: {}", file_modal.file_path));
            }

            ui.horizontal(|ui| {
                if ui.add(Button::new("Confirm")).clicked() {
                    if !file_modal.file_title.is_empty() && !file_modal.file_path.is_empty() {
                        file_modal.source_file = Some(SourceFile {
                            id: 3,
                            source: "SRT".to_string(),
                            file_type: FileType::SRT,
                            title: file_modal.file_title.clone(),
                            creator: if file_modal.file_creator.is_empty() {
                                None
                            } else {
                                Some(file_modal.file_creator.clone())
                            },
                            original_file: file_modal.file_path.clone(),
                        });

                        file_modal.file_title.clear();
                        file_modal.file_creator.clear();
                        file_modal.file_path.clear();
                        file_modal.open = false;
                    }
                    file_modal.open = false;
                }

                if ui.button("Cancel").clicked() {
                    file_modal.open = false;
                }
            });
        });
}
