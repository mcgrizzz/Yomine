use std::{
    collections::HashMap,
    sync::Arc,
};

use eframe::egui;
use vibrato::Tokenizer;

use super::{
    error_modal::ErrorModal,
    file_modal::FileModal,
    message_overlay::MessageOverlay,
    restart_modal::RestartModal,
    settings::{
        SettingsData,
        SettingsModal,
    },
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
        tasks::{
            TaskManager,
            TaskResult,
        },
        Sentence,
        SourceFile,
        Term,
    },
    dictionary::frequency_manager::FrequencyManager,
    persistence::{
        load_json_or_default,
        save_json,
    },
};

#[derive(Clone)]
pub struct LanguageTools {
    pub tokenizer: Arc<Tokenizer>,
    pub frequency_manager: Arc<FrequencyManager>,
}

impl std::fmt::Debug for LanguageTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageTools")
            .field("tokenizer", &"Arc<Tokenizer>")
            .field("frequency_manager", &"Arc<FrequencyManager>")
            .finish()
    }
}

pub struct YomineApp {
    pub terms: Vec<Term>,
    pub sentences: Vec<Sentence>,
    pub model_mapping: HashMap<String, FieldMapping>,
    pub settings_data: SettingsData,
    pub table_state: TableState,
    pub file_modal: FileModal,
    pub error_modal: ErrorModal,
    pub settings_modal: SettingsModal,
    pub restart_modal: RestartModal,
    pub theme: Theme,
    pub zoom: f32,
    pub anki_connected: bool,
    pub last_anki_check: Option<std::time::Instant>,
    pub websocket_manager: WebSocketManager,
    pub message_overlay: MessageOverlay,
    pub language_tools: Option<LanguageTools>,
    pub current_processing_file: Option<String>,
    pub current_source_file: Option<SourceFile>,
    task_manager: TaskManager,
}

impl YomineApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model_mapping: HashMap<String, FieldMapping>,
    ) -> Self {
        let task_manager = TaskManager::new();

        task_manager.load_language_tools();

        let mut settings_data = load_json_or_default::<SettingsData>("settings.json");

        for (model_name, field_mapping) in model_mapping {
            settings_data.anki_model_mappings.insert(model_name, field_mapping);
        }
        let app = Self {
            model_mapping: settings_data.anki_model_mappings.clone(),
            settings_data,
            theme: Theme::dracula(),
            zoom: cc.egui_ctx.zoom_factor(),
            anki_connected: false,
            last_anki_check: None,
            websocket_manager: WebSocketManager::new(),
            message_overlay: MessageOverlay::new(),
            file_modal: FileModal::new(),
            error_modal: ErrorModal::new(),
            settings_modal: SettingsModal::new(),
            restart_modal: RestartModal::new(),
            table_state: TableState::default(),
            language_tools: None,
            terms: Vec::new(),
            sentences: Vec::new(),
            current_processing_file: None,
            current_source_file: None,
            task_manager: task_manager,
        };

        app.setup_fonts(cc);
        app.setup_theme(cc);

        //Make sure it opens above other windows so you can see it.
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));

        app
    }
    fn setup_fonts(&self, cc: &eframe::CreationContext<'_>) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "noto_sans_jp".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/NotoSansJP-Regular.ttf"
            ))),
        );

        // fonts.font_data.insert(
        //     "noto_sans_jp_bold".to_owned(),
        //     egui::FontData::from_static(include_bytes!("../../assets/fonts/NotoSansJP-Bold.ttf"))
        //         .into(),
        // );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "noto_sans_jp".to_owned());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("noto_sans_jp".to_owned());

        cc.egui_ctx.set_fonts(fonts);
    }

    fn setup_theme(&self, cc: &eframe::CreationContext<'_>) {
        cc.egui_ctx.set_zoom_factor(cc.egui_ctx.zoom_factor() + 0.7);
        set_theme(&cc.egui_ctx, self.theme.clone());
    }
}

impl eframe::App for YomineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let task_results = self.task_manager.poll_results();

        for result in task_results {
            self.handle_task_result(result, ctx);
        }

        self.websocket_manager.update();
        self.update_anki_status();

        let current_settings = self.get_current_settings();
        TopBar::show(
            ctx,
            &mut self.file_modal,
            &mut self.settings_modal,
            &current_settings,
            &self.websocket_manager,
            self.anki_connected,
            &mut self.restart_modal,
        );
        if let Some(source_file) = self.file_modal.show(
            ctx,
            &self.theme,
            self.current_source_file.as_ref().map(|sf| sf.original_file.as_str()),
        ) {
            println!("File selected: {:?}", source_file.original_file);
            self.process_source_file(source_file);
        }

        term_table(ctx, self);
        self.message_overlay.show(ctx, &self.theme);
        self.error_modal.show(ctx); // Handle restart modal
        if let Some(should_restart) = self.restart_modal.show(ctx) {
            if should_restart {
                self.restart_application(ctx);
            }
        }

        if let Some(settings) = self.settings_modal.show(ctx) {
            self.model_mapping = settings.anki_model_mappings.clone();
            self.settings_data = settings;

            self.save_settings();
        }
    }
}

impl YomineApp {
    fn process_source_file(&mut self, source_file: SourceFile) {
        println!("Processing file: {}", source_file.original_file);

        self.current_source_file = Some(source_file.clone());

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
            let model_mapping = self.model_mapping.clone();
            self.task_manager.process_file(
                source_file_clone,
                model_mapping,
                language_tools.clone(),
            );
        } else {
            println!("Language tools not loaded yet!");
        }
    }

    fn handle_task_result(&mut self, result: TaskResult, _ctx: &egui::Context) {
        match result {
            TaskResult::LanguageToolsLoaded(result) => match result {
                Ok(language_tools) => {
                    self.language_tools = Some(language_tools);
                    self.message_overlay.clear_message();
                }
                Err(e) => {
                    self.message_overlay
                        .set_message(format!("Failed to load language tools: {}", e));
                }
            },

            TaskResult::FileProcessing(result) => {
                self.message_overlay.clear_message();
                self.handle_processing_result(result);
            }

            TaskResult::LoadingMessage(message) => {
                self.message_overlay.set_message(message);
            }

            TaskResult::AnkiConnection(connected) => {
                self.anki_connected = connected;
            }
            _ => {}
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

                if let Some(source_file) = &self.current_source_file {
                    self.file_modal.add_recent_file(
                        source_file.original_file.clone(),
                        source_file.title.clone(),
                        source_file.creator.clone(),
                        self.terms.len(),
                    );
                }
            }
            Err(error_msg) => {
                self.error_modal.show_error(
                    "File Load Error".to_string(),
                    &format!("Unable to load file: {}", filename),
                    Some(&error_msg),
                );
                self.terms = Vec::new();
                self.sentences = Vec::new();
                self.current_source_file = None;
            }
        }

        self.current_processing_file = None;
    }

    fn update_anki_status(&mut self) {
        let now = std::time::Instant::now();
        let should_check = match self.last_anki_check {
            None => true,
            Some(last_check) => now.duration_since(last_check).as_secs() >= 5,
        };

        if should_check {
            self.task_manager.check_anki_connection();
            self.last_anki_check = Some(now);
        }
    }

    fn get_current_settings(&self) -> SettingsData {
        self.settings_data.clone()
    }
    fn save_settings(&self) {
        if let Err(e) = save_json(&self.settings_data, "settings.json") {
            eprintln!("Failed to save settings: {}", e);
        }
    }

    fn restart_application(&self, ctx: &egui::Context) {
        // Get the current executable path
        if let Ok(current_exe) = std::env::current_exe() {
            // Start a new instance of the application
            if let Err(e) = std::process::Command::new(&current_exe).spawn() {
                eprintln!("Failed to restart application: {}", e);
            } else {
                // Close the current instance
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else {
            eprintln!("Failed to get current executable path for restart");
            // Fallback to just closing the application
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
