pub mod theme;
pub mod table;

use std::collections::HashSet;
use std::sync::Arc;
use eframe::{egui::{self, Button, TextEdit}, epaint::text::{FontInsert, InsertFontFamily}};
use rfd::FileDialog;
use table::{term_table, TableState};
use theme::{set_theme, Theme};
use tokio::runtime::Runtime;

use crate::core::{Sentence, SourceFile, Term};
use crate::websocket::WebSocketServer;


struct FileModal {
    pub open: bool,
    pub file_title: String,
    pub file_creator: String,
    pub file_path: String,
    pub source_file: Option<SourceFile>,
}

impl Default for FileModal {
    fn default() -> Self {
        Self { open: Default::default(), file_title: Default::default(), file_creator: Default::default(), file_path: Default::default(), source_file: Default::default() }
    }
}

#[derive(Default)]
pub struct WebSocketState {
    has_clients: bool,
    confirmed_timestamps: Vec<String>,  // Store timestamps that have been confirmed
}

#[derive(Default)]
pub struct YomineApp {
    terms: Vec<Term>,
    sentences: Vec<Sentence>,
    table_state: TableState,
    file_modal: FileModal,
    zoom: f32,
    theme: Theme,
    websocket_state: WebSocketState,
    websocket_server: Option<Arc<WebSocketServer>>,
}



impl YomineApp {
    pub fn new(cc: &eframe::CreationContext<'_>, terms: Vec<Term>, sentences: Vec<Sentence>) -> Self {
        // Start the WebSocket server at startup
        let websocket_server = WebSocketServer::start_server();
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        cc.egui_ctx.add_font(FontInsert::new(
            "Noto Sans",
            egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/NotoSansJP-Regular.ttf"
            )),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));

        cc.egui_ctx.add_font(FontInsert::new(
            "Noto Sans",
            egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/NotoSansJP-Bold.ttf"
            )),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));
        
        let mut seen = HashSet::new();

        let mut terms: Vec<Term> = terms
            .into_iter()
            .filter(|term| seen.insert((term.lemma_form.clone(), term.lemma_reading.clone())))
            .collect();

        set_theme(&cc.egui_ctx, Theme::dracula());
        cc.egui_ctx.set_zoom_factor(cc.egui_ctx.zoom_factor() + 0.7);
        cc.egui_ctx.style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });

        terms.sort_unstable_by(|a, b| {
            let freq_a = a.frequencies.get("HARMONIC").unwrap();
            let freq_b = b.frequencies.get("HARMONIC").unwrap();
            freq_a.cmp(freq_b)
        });
            
        Self {
            terms,
            sentences,
            table_state: TableState::default(),
            file_modal: FileModal::default(),
            zoom: cc.egui_ctx.zoom_factor(),
            theme: Theme::dracula(),
            websocket_state: WebSocketState { 
                has_clients: false, // Will update in the update method
                confirmed_timestamps: Vec::new(),
            },
            websocket_server,
        }
    }
    
}

impl eframe::App for YomineApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update WebSocket status and confirmed timestamps
        if let Some(server) = &self.websocket_server {
            self.websocket_state.has_clients = server.has_clients();
            
            // Process any new confirmations from ASBPlayer
            // This is now a quick single-channel operation without internal broadcasts
            server.process_pending_confirmations();
            
            // Update our list of confirmed timestamps
            if self.websocket_state.has_clients {
                // Get the latest confirmed timestamps from the server
                self.websocket_state.confirmed_timestamps = server.get_confirmed_timestamps();
            }
        }
        
        self.top_bar(ctx);

        if self.file_modal.open {
            open_new_file_dialog(ctx, &mut self.file_modal);
        }

        term_table(ctx, self);
    }
    
}

impl YomineApp {

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
    
            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                ui.menu_button("File", |ui| {
                    if ui.button("Open New File").clicked() {
                        self.file_modal.open = true;
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);
    
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });
        
    }
}

fn open_new_file_dialog(ctx: &egui::Context, file_modal: &mut FileModal) {
    egui::Window::new("Open New File")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Enter Title:");
            ui.add(TextEdit::singleline(&mut file_modal.file_title).hint_text("E.g., Dan Da Dan - S01E08"));

            ui.label("Enter Creator (optional):");
            ui.add(TextEdit::singleline(&mut file_modal.file_creator).hint_text("E.g., Netflix"));

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
                            title: file_modal.file_title.clone(),
                            creator: if file_modal.file_creator.is_empty() {
                                None
                            } else {
                                Some(file_modal.file_creator.clone())
                            },
                            original_file: file_modal.file_path.clone(),
                        });

                        // Reset form fields
                        file_modal.file_title.clear();
                        file_modal.file_creator.clear();
                        file_modal.file_path.clear();
                        file_modal.open = false; // Close modal
                    }
                    file_modal.open = false; // Close modal
                }

                if ui.button("Cancel").clicked() {
                    file_modal.open = false; // Close modal
                }
            });
        });
}

