use std::{
    collections::HashMap,
    sync::{
        mpsc::{
            Receiver,
            Sender,
        },
        Arc,
    },
};

use eframe::egui;
use vibrato::Tokenizer;

use super::{
    error_modal::ErrorModal,
    file_modal::FileModal,
    message_overlay::MessageOverlay,
    table::{
        term_table,
        TableState,
    },
    theme::{
        set_theme,
        Theme,
    },
    top_bar::TopBar,
    websocket_manager::WebSocketManager,
};
use crate::{
    anki::FieldMapping,
    core::{
        pipeline::process_source_file,
        Sentence,
        SourceFile,
        Term,
    },
    dictionary::frequency_manager::FrequencyManager,
};

pub struct LanguageTools {
    pub tokenizer: Arc<Tokenizer>,
    pub frequency_manager: Arc<FrequencyManager>,
}

#[derive(Default)]
pub struct YomineApp {
    pub terms: Vec<Term>,
    pub sentences: Vec<Sentence>,
    pub model_mapping: HashMap<String, FieldMapping>,
    pub table_state: TableState,
    pub file_modal: FileModal,
    pub error_modal: ErrorModal,
    pub theme: Theme,
    pub zoom: f32,
    pub anki_connected: bool,
    pub last_anki_check: Option<std::time::Instant>,
    pub websocket_manager: WebSocketManager,
    pub message_overlay: MessageOverlay,
    pub language_tools: Option<LanguageTools>,
    pub language_tools_receiver: Option<Receiver<Result<LanguageTools, String>>>,
    pub loading_message_receiver: Option<Receiver<String>>,
    pub update_receiver: Option<Receiver<Result<(Vec<Term>, Vec<Sentence>), String>>>,
    pub current_processing_file: Option<String>,
}

impl YomineApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model_mapping: HashMap<String, FieldMapping>,
    ) -> Self {
        let (language_tools_sender, language_tools_receiver) = std::sync::mpsc::channel();
        let (loading_message_sender, loading_message_receiver) = std::sync::mpsc::channel();

        Self::start_background_loading(language_tools_sender, loading_message_sender);
        let app = Self {
            model_mapping,
            theme: Theme::dracula(),
            zoom: cc.egui_ctx.zoom_factor(),
            anki_connected: false,
            last_anki_check: None,
            websocket_manager: WebSocketManager::new(),
            message_overlay: MessageOverlay::new(),
            file_modal: FileModal::new(),
            error_modal: ErrorModal::new(),
            table_state: TableState::default(),
            language_tools: None,
            language_tools_receiver: Some(language_tools_receiver),
            loading_message_receiver: Some(loading_message_receiver),
            terms: Vec::new(),
            sentences: Vec::new(),
            update_receiver: None,
            current_processing_file: None,
        };

        app.setup_fonts(cc);
        app.setup_theme(cc);

        app
    }
    fn setup_fonts(&self, cc: &eframe::CreationContext<'_>) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "noto_sans_jp".to_owned(),
            egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/NotoSansJP-Regular.ttf"
            ))
            .into(),
        );

        fonts.font_data.insert(
            "noto_sans_jp_bold".to_owned(),
            egui::FontData::from_static(include_bytes!("../../assets/fonts/NotoSansJP-Bold.ttf"))
                .into(),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "noto_sans_jp".to_owned());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "noto_sans_jp".to_owned());

        cc.egui_ctx.set_fonts(fonts);
    }

    fn setup_theme(&self, cc: &eframe::CreationContext<'_>) {
        cc.egui_ctx.set_zoom_factor(cc.egui_ctx.zoom_factor() + 0.7);
        set_theme(&cc.egui_ctx, self.theme.clone());
    }

    fn start_background_loading(
        language_tools_sender: Sender<Result<LanguageTools, String>>,
        loading_message_sender: Sender<String>,
    ) {
        std::thread::spawn(move || {
            use std::sync::Arc;

            use crate::{
                dictionary::{
                    frequency_manager,
                    token_dictionary::DictType,
                },
                segmentation::tokenizer::init_vibrato,
            };

            let result = (|| -> Result<LanguageTools, String> {
                let _ = loading_message_sender.send("Loading tokenizer...".to_string());
                let dict_type = DictType::Unidic;
                let tokenizer = Arc::new(init_vibrato(&dict_type).map_err(|e| e.to_string())?);

                let _ =
                    loading_message_sender.send("Loading frequency dictionaries...".to_string());
                let frequency_manager = Arc::new(
                    frequency_manager::process_frequency_dictionaries()
                        .map_err(|e| e.to_string())?,
                );

                let _ =
                    loading_message_sender.send("Language tools loaded successfully!".to_string());
                Ok(LanguageTools { tokenizer, frequency_manager })
            })();

            language_tools_sender.send(result).unwrap();
        });
    }
}

impl eframe::App for YomineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(receiver) = &self.loading_message_receiver {
            if let Ok(message) = receiver.try_recv() {
                self.message_overlay.set_message(message);
                ctx.request_repaint();
            }
        }

        if let Some(receiver) = &self.language_tools_receiver {
            if let Ok(result) = receiver.try_recv() {
                match &result {
                    Ok(language_tools) => {
                        self.language_tools = Some(LanguageTools {
                            tokenizer: language_tools.tokenizer.clone(),
                            frequency_manager: language_tools.frequency_manager.clone(),
                        });
                        self.message_overlay.clear_message();
                        self.loading_message_receiver = None;
                    }
                    Err(_) => {
                        self.message_overlay.set_message(
                            "Failed to load language tools. Please check your dictionaries."
                                .to_string(),
                        );
                        self.loading_message_receiver = None;
                    }
                }
                self.language_tools_receiver = None;
                ctx.request_repaint();
            }
        }

        self.websocket_manager.update();
        self.update_anki_status();
        TopBar::show(ctx, &mut self.file_modal, &self.websocket_manager, self.anki_connected);
        if let Some(source_file) = self.file_modal.show(ctx) {
            println!("File selected: {:?}", source_file.original_file);
            self.process_source_file(source_file);
        }

        if let Some(receiver) = &self.update_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.handle_processing_result(result);
                ctx.request_repaint();
            }
        }
        term_table(ctx, self);

        self.message_overlay.show(ctx, &self.theme);
        self.error_modal.show(ctx);
    }
}

impl YomineApp {
    fn process_source_file(&mut self, source_file: SourceFile) {
        println!("Processing file: {}", source_file.original_file);

        self.current_processing_file = Some(
            std::path::Path::new(&source_file.original_file)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown file")
                .to_string(),
        );

        if let Some(language_tools) = &self.language_tools {
            self.message_overlay.set_message("Processing file...".to_string());
            let source_file_clone = source_file.clone();
            let tokenizer = language_tools.tokenizer.clone();
            let frequency_manager = language_tools.frequency_manager.clone();
            let model_mapping = self.model_mapping.clone();
            let (sender, receiver) = std::sync::mpsc::channel();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let result = rt.block_on(async {
                    process_source_file(
                        &source_file_clone,
                        model_mapping,
                        &(LanguageTools { tokenizer, frequency_manager }),
                    )
                    .await
                    .map_err(|e| e.to_string())
                });

                sender.send(result).unwrap();
            });

            self.update_receiver = Some(receiver);
        } else {
            println!("Language tools not loaded yet!");
        }
    }

    fn handle_processing_result(&mut self, result: Result<(Vec<Term>, Vec<Sentence>), String>) {
        self.message_overlay.clear_message();
        let filename = self.current_processing_file.as_deref().unwrap_or("the selected file");

        match result {
            Ok((new_terms, new_sentences)) => {
                self.terms = new_terms;

                self.terms.sort_unstable_by(|a, b| {
                    let freq_a = a.frequencies.get("HARMONIC").unwrap_or(&0);
                    let freq_b = b.frequencies.get("HARMONIC").unwrap_or(&0);
                    freq_a.cmp(freq_b)
                });

                self.sentences = new_sentences;
            }
            Err(error_msg) => {
                self.error_modal.show_error_with_details(
                    "File Load Error".to_string(),
                    &format!("Unable to load file: {}", filename),
                    &error_msg,
                );
                self.terms = Vec::new();
                self.sentences = Vec::new();
            }
        }
        self.update_receiver = None;
        self.current_processing_file = None;
    }

    fn update_anki_status(&mut self) {
        let now = std::time::Instant::now();
        let should_check = match self.last_anki_check {
            None => true,
            Some(last_check) => now.duration_since(last_check).as_secs() >= 5,
        };

        if should_check {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                self.anki_connected =
                    rt.block_on(async { crate::anki::api::get_version().await.is_ok() });
            } else {
                self.anki_connected = false;
            }
            self.last_anki_check = Some(now);
        }
    }
}
