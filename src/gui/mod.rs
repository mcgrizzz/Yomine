pub mod theme;
pub mod table;

use std::collections::{ HashMap, HashSet };
use std::sync::Arc;
use eframe::{ egui::{ self, Button, TextEdit }, epaint::text::{ FontInsert, InsertFontFamily } };
use rfd::FileDialog;
use table::{ term_table, TableState };
use theme::{ set_theme, Theme };
use std::sync::mpsc::Receiver;
use vibrato::Tokenizer;

use crate::core::pipeline::process_source_file;
use crate::dictionary::frequency_manager::FrequencyManager;
use crate::{ anki::FieldMapping, core::{ models::FileType, Sentence, SourceFile, Term } };
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
        Self {
            open: Default::default(),
            file_title: Default::default(),
            file_creator: Default::default(),
            file_path: Default::default(),
            source_file: Default::default(),
        }
    }
}

#[derive(Default)]
pub struct WebSocketState {
    has_clients: bool,
    confirmed_timestamps: Vec<String>, // Store timestamps that have been confirmed
}

pub struct LanguageTools {
    pub tokenizer: Arc<Tokenizer>,
    pub frequency_manager: Arc<FrequencyManager>,
}

impl Default for LanguageTools {
    fn default() -> Self {
        panic!(
            "LanguageTools requires explicit initialization with valid references to Tokenizer and FrequencyManager."
        )
    }
}

#[derive(Default)]
pub struct YomineApp {
    terms: Vec<Term>,
    sentences: Vec<Sentence>,
    table_state: TableState,
    loading_state: bool,
    file_modal: FileModal,
    zoom: f32,
    theme: Theme,
    websocket_state: WebSocketState,
    websocket_server: Option<Arc<WebSocketServer>>,
    language_tools: LanguageTools,
    model_mapping: HashMap<String, FieldMapping>,
    update_receiver: Option<Receiver<Result<(Vec<Term>, Vec<Sentence>), String>>>,
}

impl YomineApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        terms: Vec<Term>,
        sentences: Vec<Sentence>,
        model_mapping: HashMap<String, FieldMapping>,
        language_tools: LanguageTools
    ) -> Self {
        // Start the WebSocket server at startup
        let websocket_server = WebSocketServer::start_server();
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        cc.egui_ctx.add_font(
            FontInsert::new(
                "Noto Sans",
                egui::FontData::from_static(
                    include_bytes!("../../assets/fonts/NotoSansJP-Regular.ttf")
                ),
                vec![
                    InsertFontFamily {
                        family: egui::FontFamily::Proportional,
                        priority: egui::epaint::text::FontPriority::Highest,
                    },
                    InsertFontFamily {
                        family: egui::FontFamily::Monospace,
                        priority: egui::epaint::text::FontPriority::Lowest,
                    }
                ]
            )
        );

        cc.egui_ctx.add_font(
            FontInsert::new(
                "Noto Sans",
                egui::FontData::from_static(
                    include_bytes!("../../assets/fonts/NotoSansJP-Bold.ttf")
                ),
                vec![
                    InsertFontFamily {
                        family: egui::FontFamily::Proportional,
                        priority: egui::epaint::text::FontPriority::Highest,
                    },
                    InsertFontFamily {
                        family: egui::FontFamily::Monospace,
                        priority: egui::epaint::text::FontPriority::Lowest,
                    }
                ]
            )
        );

        cc.egui_ctx.set_zoom_factor(cc.egui_ctx.zoom_factor() + 0.7);

        let mut seen = HashSet::new();

        let mut terms: Vec<Term> = terms
            .into_iter()
            .filter(|term| seen.insert((term.lemma_form.clone(), term.lemma_reading.clone())))
            .collect();

        set_theme(&cc.egui_ctx, Theme::dracula());

        terms.sort_unstable_by(|a, b| {
            let freq_a = a.frequencies.get("HARMONIC").unwrap();
            let freq_b = b.frequencies.get("HARMONIC").unwrap();
            freq_a.cmp(freq_b)
        });

        Self {
            terms,
            sentences,
            table_state: TableState::default(),
            loading_state: false,
            file_modal: FileModal::default(),
            zoom: cc.egui_ctx.zoom_factor(),
            theme: Theme::dracula(),
            websocket_state: WebSocketState {
                has_clients: false, // Will update in the update method
                confirmed_timestamps: Vec::new(),
            },
            websocket_server,
            language_tools,
            model_mapping,
            update_receiver: None,
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
            if let Some(source_file) = &self.file_modal.source_file {
                self.loading_state = true;
                let source_file_clone = source_file.clone();
                let tokenizer = self.language_tools.tokenizer.clone();
                let frequency_manager = self.language_tools.frequency_manager.clone();
                let model_mapping = self.model_mapping.clone();

                // Create a channel for communication between threads
                let (sender, receiver) = std::sync::mpsc::channel();

                // Spawn a new thread to process the file
                std::thread::spawn(move || {
                    // Create a tokio runtime for the async call
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let result = rt.block_on(async { process_source_file(
                            &source_file_clone,
                            model_mapping,
                            &(LanguageTools {
                                tokenizer,
                                frequency_manager,
                            })
                        ).await.map_err(|e| e.to_string()) }); // Convert error to string

                    sender.send(result).unwrap();
                });

                // Store the receiver to check for updates later
                self.update_receiver = Some(receiver);
                self.file_modal.source_file = None; // Reset to avoid re-processing
            }
        }

        if let Some(receiver) = &self.update_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.loading_state = false; // Loading is complete
                match result {
                    Ok((new_terms, new_sentences)) => {
                        self.terms = new_terms; // Update with new data

                        self.terms.sort_unstable_by(|a, b| {
                            let freq_a = a.frequencies.get("HARMONIC").unwrap_or(&0);
                            let freq_b = b.frequencies.get("HARMONIC").unwrap_or(&0);
                            freq_a.cmp(freq_b)
                        });

                        self.sentences = new_sentences;
                    }

                    Err(e) => {
                        self.terms = Vec::new(); // Empty terms on error
                        self.sentences = Vec::new(); // Empty sentences on error
                    }
                }
                self.update_receiver = None; // Clear receiver after processing
                ctx.request_repaint(); // Refresh UI
            }
        }

        term_table(ctx, self);

        if self.loading_state {
            egui::Area
                ::new(egui::Id::new("loading_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::Pos2::new(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen_size = ui.ctx().screen_rect().size();
                    ui.allocate_space(screen_size);
                    ui.painter().rect_filled(
                        ui.ctx().screen_rect(),
                        0.0,
                        egui::Color32::from_black_alpha(120)
                    );
                });

            egui::Window
                ::new("Loading")
                .order(egui::Order::Foreground)
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .fixed_size(egui::Vec2::new(200.0, 100.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::new(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.style_mut().visuals.window_fill = egui::Color32::from_rgba_premultiplied(
                        0,
                        0,
                        0,
                        150
                    );
                    ui.style_mut().visuals.window_stroke = egui::Stroke::new(2.0, self.theme.red());

                    ui.centered_and_justified(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label("Loading file...");
                    });
                });
        }
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
