mod file_data;
mod modals;

use std::{
    collections::HashMap,
    mem,
    sync::{
        Arc,
        Mutex,
    },
};

use eframe::egui::{
    self,
    Id,
};
use file_data::FileData;
use modals::Modals;
use vibrato::Tokenizer;

use super::{
    file_modal::FileModal,
    message_overlay::MessageOverlay,
    settings::{
        data::FrequencyDictionarySetting,
        SettingsData,
    },
    setup_banner::SetupBanner,
    setup_checklist_modal::SetupAction,
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
        models::SourceFileType,
        tasks::{
            types::FrequencyAnalysisUpdate,
            TaskManager,
            TaskResult,
        },
        IgnoreList,
        Sentence,
        SourceFile,
        Term,
    },
    dictionary::{
        frequency_manager::FrequencyManager,
        frequency_utils,
    },
    persistence::{
        load_json_or_default,
        save_json,
    },
    player::PlayerManager,
};

#[derive(Clone)]
pub struct LanguageTools {
    pub tokenizer: Arc<Tokenizer>,
    pub frequency_manager: Arc<FrequencyManager>,
    pub ignore_list: Arc<Mutex<IgnoreList>>,
    pub known_interval: u32,
}

impl std::fmt::Debug for LanguageTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageTools")
            .field("tokenizer", &"Arc<Tokenizer>")
            .field("frequency_manager", &"Arc<FrequencyManager>")
            .field("ignore_list", &"Arc<Mutex<IgnoreList>>")
            .finish()
    }
}

pub struct YomineApp {
    // File Data
    pub file_data: Option<FileData>,

    // Configuration
    pub settings_data: SettingsData,

    // UI State
    pub table_state: TableState,
    pub theme: Theme,
    pub zoom: f32,
    pub message_overlay: MessageOverlay,

    // Modals
    pub modals: Modals,

    // External Services
    pub language_tools: Option<LanguageTools>,
    pub player: PlayerManager,
    pub anki_connected: bool,
    pub last_anki_check: Option<std::time::Instant>,
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
        let mut table_state = TableState::default();
        table_state.apply_pos_settings(&settings_data.pos_filters);

        for (model_name, field_mapping) in model_mapping {
            settings_data.anki_model_mappings.insert(model_name, field_mapping);
        }

        // Initialize PlayerManager with both WebSocket and MPV managers
        let websocket_manager = WebSocketManager::new(settings_data.websocket_settings.port);
        let mpv_manager = crate::mpv::MpvManager::new();
        let player = PlayerManager::new(mpv_manager, websocket_manager);

        let app = Self {
            // File Data
            file_data: None,

            // Configuration
            settings_data,

            // UI State
            table_state,
            theme: Theme::dracula(),
            zoom: cc.egui_ctx.zoom_factor(),
            message_overlay: MessageOverlay::new(),

            // Modals
            modals: Modals::default(),

            // External Services
            language_tools: None,
            player,
            anki_connected: false,
            last_anki_check: None,
            task_manager,
        };

        app.setup_fonts(cc);
        app.setup_theme(cc);

        // Apply saved font preference
        apply_font_family(&cc.egui_ctx, app.settings_data.use_serif_font);

        // Apply saved theme preference (set_theme switches to the registered variant)
        cc.egui_ctx.set_theme(if app.settings_data.dark_mode {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });

        cc.egui_ctx.options_mut(|o| {
            o.theme_preference = if app.settings_data.dark_mode {
                egui::ThemePreference::Dark
            } else {
                egui::ThemePreference::Light
            };
        });

        //Make sure it opens above other windows so you can see it.
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));

        app
    }
    fn setup_fonts(&self, cc: &eframe::CreationContext<'_>) {
        let mut fonts = egui::FontDefinitions::default();

        // Register Noto Sans JP
        fonts.font_data.insert(
            "noto_sans_jp".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../../../assets/fonts/NotoSansJP-Regular.ttf"
            ))),
        );

        // Register Noto Serif JP
        fonts.font_data.insert(
            "noto_serif_jp".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../../../assets/fonts/NotoSerifJP-Regular.ttf"
            ))),
        );

        // Get default egui fonts for fallback (they contain special symbols)
        let default_fonts = egui::FontDefinitions::default();

        // Create named font families for Sans with default fonts as fallback
        let sans_family =
            fonts.families.entry(egui::FontFamily::Name("noto_sans_jp".into())).or_default();
        sans_family.insert(0, "noto_sans_jp".to_owned());
        // Add default fonts for symbols
        if let Some(default_proportional) =
            default_fonts.families.get(&egui::FontFamily::Proportional)
        {
            for (i, font) in default_proportional.iter().enumerate() {
                sans_family.insert(i + 1, font.clone());
            }
        }

        // Create named font families for Serif with Sans and default fonts as fallback
        let serif_family =
            fonts.families.entry(egui::FontFamily::Name("noto_serif_jp".into())).or_default();
        serif_family.insert(0, "noto_serif_jp".to_owned());
        serif_family.insert(1, "noto_sans_jp".to_owned());

        if let Some(default_proportional) =
            default_fonts.families.get(&egui::FontFamily::Proportional)
        {
            for (i, font) in default_proportional.iter().enumerate() {
                serif_family.insert(i + 2, font.clone());
            }
        }

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "noto_sans_jp".to_owned());

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(1, "noto_serif_jp".to_owned());

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

pub fn apply_font_family(ctx: &egui::Context, use_serif: bool) {
    ctx.all_styles_mut(|style| {
        for (_text_style, font_id) in style.text_styles.iter_mut() {
            font_id.family = egui::FontFamily::Name(
                if use_serif { "noto_serif_jp" } else { "noto_sans_jp" }.into(),
            );
        }
    });
}

impl eframe::App for YomineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let task_results = self.task_manager.poll_results();

        for result in task_results {
            self.handle_task_result(result, ctx);
        }

        // Update the combined player (handles both MPV and WebSocket)
        self.player.update(self.settings_data.websocket_settings.port);
        self.update_anki_status();
        self.handle_file_drops(ctx);
        self.draw_file_drop_overlay(ctx);

        let ignore_list_ref = self.language_tools.as_ref().map(|lt| &lt.ignore_list);
        let frequency_manager =
            self.language_tools.as_ref().map(|lt| lt.frequency_manager.as_ref());

        let can_refresh = self.file_data.as_ref().map_or(false, |fd| !fd.original_terms.is_empty())
            && self.language_tools.is_some();

        TopBar::show(
            ctx,
            &mut self.modals.file,
            &mut self.modals.anki_settings,
            &mut self.modals.websocket_settings,
            &mut self.modals.ignore_list,
            &mut self.modals.frequency_weights,
            &mut self.modals.pos_filters,
            &mut self.modals.frequency_analyzer,
            &mut self.modals.setup_checklist,
            &mut self.modals.restart,
            &mut self.settings_data,
            &self.player.ws,
            self.player.mpv.is_connected(),
            self.anki_connected,
            ignore_list_ref,
            &self.task_manager,
            can_refresh,
            &self.table_state,
            frequency_manager,
        );

        let banner_clicked =
            SetupBanner::show(ctx, self.language_tools.as_ref(), &self.settings_data);

        if banner_clicked {
            self.modals.setup_checklist.open_modal();
        }

        if let Some(source_file) = self.modals.file.show(
            ctx,
            &self.theme,
            self.file_data.as_ref().map(|fd| fd.source_file.original_file.as_str()),
        ) {
            println!("File selected: {:?}", source_file.original_file);
            self.process_source_file(source_file);
        }

        term_table(ctx, self);
        self.message_overlay.show(ctx, &self.theme);
        self.modals.error.show(ctx); // Handle restart modal
        if let Some(should_restart) = self.modals.restart.show(ctx) {
            if should_restart {
                self.restart_application(ctx);
            }
        }

        if let Some(settings) = self.modals.anki_settings.show(ctx) {
            self.settings_data = settings;

            if let Some(language_tools) = &mut self.language_tools {
                language_tools.known_interval = self.settings_data.anki_interval;
            }

            self.save_settings();
        }

        if let Some(settings) = self.modals.websocket_settings.show(ctx, &mut self.player.ws) {
            self.settings_data = settings;
            self.save_settings();
        }

        if let Some(weights) = self.modals.frequency_weights.show(ctx) {
            self.settings_data.frequency_weights = weights;
            if let Some(manager) =
                self.language_tools.as_ref().map(|lt| Arc::clone(&lt.frequency_manager))
            {
                self.apply_frequency_settings(manager.as_ref());
            }
            self.save_settings();
        }

        if let Some(pos_settings) = self.modals.pos_filters.show(ctx) {
            self.settings_data.pos_filters = pos_settings;
            self.table_state.apply_pos_settings(&self.settings_data.pos_filters);
            if let Some(file_data) = &self.file_data {
                let freq_manager =
                    self.language_tools.as_ref().map(|lt| lt.frequency_manager.as_ref());
                self.table_state.ensure_indices(
                    &file_data.terms,
                    &file_data.sentences,
                    freq_manager,
                );
            }
            self.save_settings();
        }

        if let Some(action) = self.modals.setup_checklist.show(
            ctx,
            self.language_tools.as_ref(),
            self.anki_connected,
            &self.player,
            &self.settings_data,
        ) {
            match action {
                SetupAction::OpenUrl(url) => {
                    let _ = open::that(&url);
                }
                SetupAction::OpenAnkiSettings => {
                    self.modals.anki_settings.open_settings(self.settings_data.clone(), ctx);
                }
                SetupAction::LoadFrequencyDictionary => {
                    frequency_utils::load_frequency_dictionaries(&mut self.modals.restart);
                }
                SetupAction::OpenWebSocketSettings => {
                    self.modals.websocket_settings.open_settings(self.settings_data.clone());
                }
            }
        }

        if let Some(language_tools) = &self.language_tools {
            let ignore_list_changed =
                self.modals.ignore_list.show(ctx, &language_tools.ignore_list);
            if ignore_list_changed {
                self.partial_refresh();
            }
        }

        self.modals.frequency_analyzer.show(ctx, self.language_tools.as_ref(), &self.task_manager);
    }
}

impl YomineApp {
    fn process_source_file(&mut self, source_file: SourceFile) {
        println!("Processing file: {}", source_file.original_file);

        let processing_filename = std::path::Path::new(&source_file.original_file)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown file")
            .to_string();

        // Create FileData with source info and placeholders for processing results
        self.file_data = Some(FileData {
            source_file: source_file.clone(),
            processing_filename: Some(processing_filename),
            terms: Vec::new(),
            original_terms: Vec::new(),
            anki_filtered_terms: Default::default(),
            sentences: Vec::new(),
            file_comprehension: 0.0,
        });

        if let Some(language_tools) = &self.language_tools {
            self.message_overlay.set_message("Processing file...".to_string());
            let model_mapping = self.settings_data.anki_model_mappings.clone();
            self.task_manager.process_file(source_file, model_mapping, language_tools.clone());
        } else {
            println!("Language tools not loaded yet!");
        }
    }

    fn handle_task_result(&mut self, result: TaskResult, ctx: &egui::Context) {
        match result {
            TaskResult::LanguageToolsLoaded(result) => match result {
                Ok(mut language_tools) => {
                    // Update known_interval from settings
                    language_tools.known_interval = self.settings_data.anki_interval;

                    self.language_tools = Some(language_tools);
                    self.message_overlay.clear_message();
                    if let Some(manager) =
                        self.language_tools.as_ref().map(|lt| Arc::clone(&lt.frequency_manager))
                    {
                        self.apply_frequency_settings(manager.as_ref());
                    }
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

            TaskResult::TermsRefreshed(result) => {
                self.message_overlay.clear_message();
                match result {
                    Ok((filter_result, sentences, file_comprehension)) => {
                        if let Some(file_data) = &mut self.file_data {
                            // Reconstruct original_terms from all three sets
                            let mut base_terms = Vec::new();
                            base_terms.extend(filter_result.terms.iter().cloned());
                            base_terms.extend(filter_result.anki_filtered.iter().cloned());
                            base_terms.extend(filter_result.ignore_filtered.iter().cloned());

                            file_data.original_terms = base_terms;
                            file_data.anki_filtered_terms = filter_result
                                .anki_filtered
                                .iter()
                                .map(|t| t.lemma_form.clone())
                                .collect();
                            file_data.terms = filter_result.terms;
                            file_data.sentences = sentences;
                            file_data.file_comprehension = file_comprehension;

                            self.table_state.configure_bounds(&file_data.terms);
                            let freq_manager = self
                                .language_tools
                                .as_ref()
                                .map(|lt| lt.frequency_manager.as_ref());
                            self.table_state.ensure_indices(
                                &file_data.terms,
                                &file_data.sentences,
                                freq_manager,
                            );
                        }
                    }
                    Err(error_msg) => {
                        self.modals.error.show_error(
                            "Refresh Error".to_string(),
                            "Unable to refresh terms".to_string().as_str(),
                            Some(&error_msg),
                        );
                    }
                }
            }

            TaskResult::LoadingMessage(message) => {
                self.message_overlay.set_message(message);
            }

            TaskResult::AnkiConnection(connected) => {
                self.anki_connected = connected;
            }
            TaskResult::RequestRefresh => {
                if let Some(file_data) = &self.file_data {
                    if !file_data.original_terms.is_empty() {
                        if let Some(language_tools) = self.language_tools.clone() {
                            self.message_overlay.set_message("Refreshing terms...".to_string());
                            self.task_manager.refresh_terms(
                                file_data.original_terms.clone(),
                                file_data.sentences.clone(),
                                self.settings_data.anki_model_mappings.clone(),
                                language_tools,
                            );
                        }
                    }
                }
            }
            TaskResult::RequestSaveSettings => {
                self.save_settings();
            }
            TaskResult::FrequencyAnalysis(update) => {
                match update {
                    FrequencyAnalysisUpdate::Progress(progress) => {
                        self.modals.frequency_analyzer.handle_analysis_progress(
                            progress.current_file,
                            progress.message,
                            progress.file_size,
                        );
                        ctx.request_repaint();
                    }
                    FrequencyAnalysisUpdate::Complete(result) => {
                        self.message_overlay.clear_message();
                        self.modals.frequency_analyzer.handle_analysis_complete(result);
                    }
                    FrequencyAnalysisUpdate::Cancelled => {
                        // Task was cancelled, modal already updated its state
                        self.message_overlay.clear_message();
                    }
                }
            }
            TaskResult::FrequencyExport(result) => {
                self.modals.frequency_analyzer.handle_export_complete(result);
            }
            _ => {}
        }
    }

    pub fn partial_refresh(&mut self) {
        // Apply ignore list + cached Anki filter (no Anki connection)
        if let Some(file_data) = &mut self.file_data {
            if let Some(language_tools) = &self.language_tools {
                // Use async block since apply_filters is async, but won't actually await anything
                // because we're passing cached Anki terms
                let rt = tokio::runtime::Runtime::new().unwrap();
                match rt.block_on(crate::core::pipeline::apply_filters(
                    file_data.original_terms.clone(),
                    language_tools,
                    None,
                    Some(&file_data.anki_filtered_terms),
                )) {
                    Ok(filter_result) => {
                        file_data.terms = filter_result.terms;
                        self.table_state.configure_bounds(&file_data.terms);
                        let freq_manager = language_tools.frequency_manager.as_ref();
                        self.table_state.ensure_indices(
                            &file_data.terms,
                            &file_data.sentences,
                            Some(freq_manager),
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to reapply filters: {}", e);
                    }
                }
            }
        }
    }

    fn handle_processing_result(
        &mut self,
        result: Result<
            (Vec<Term>, crate::core::pipeline::FilterResult, Vec<Sentence>, f32),
            String,
        >,
    ) {
        self.message_overlay.clear_message();

        match result {
            Ok((base_terms, filter_result, new_sentences, file_comprehension)) => {
                // Create FileData with all file-related state
                let source_file = self
                    .file_data
                    .as_ref()
                    .and_then(|fd| Some(fd.source_file.clone()))
                    .unwrap_or_else(|| {
                        // Fallback: shouldn't happen, but handle gracefully
                        crate::core::SourceFile {
                            id: 0,
                            source: None,
                            original_file: "Unknown".to_string(),
                            title: "Unknown".to_string(),
                            creator: None,
                            file_type: crate::core::models::SourceFileType::Other(
                                "Unknown".to_string(),
                            ),
                        }
                    });

                let file_data = FileData {
                    source_file: source_file.clone(),
                    processing_filename: None,
                    terms: filter_result.terms,
                    original_terms: base_terms,
                    anki_filtered_terms: filter_result
                        .anki_filtered
                        .iter()
                        .map(|t| t.lemma_form.clone())
                        .collect(),
                    sentences: new_sentences,
                    file_comprehension,
                };

                self.table_state.reset();
                self.table_state.configure_bounds(&file_data.terms);

                // Reapply POS filter settings after reset
                self.table_state.apply_pos_settings(&self.settings_data.pos_filters);

                let freq_manager =
                    self.language_tools.as_ref().map(|lt| lt.frequency_manager.as_ref());
                self.table_state.ensure_indices(
                    &file_data.terms,
                    &file_data.sentences,
                    freq_manager,
                );

                self.modals.file.add_recent_file(
                    source_file.original_file.clone(),
                    source_file.title.clone(),
                    source_file.creator.clone(),
                    file_data.terms.len(),
                );

                self.file_data = Some(file_data);
            }
            Err(error_msg) => {
                let filename = self
                    .file_data
                    .as_ref()
                    .and_then(|fd| fd.processing_filename.as_deref())
                    .unwrap_or("the selected file");

                self.modals.error.show_error(
                    "File Load Error".to_string(),
                    &format!("Unable to load file: {}", filename),
                    Some(&error_msg),
                );
                self.file_data = None;
                self.table_state.reset();
            }
        }
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

    fn save_settings(&self) {
        if let Err(e) = save_json(&self.settings_data, "settings.json") {
            eprintln!("Failed to save settings: {}", e);
        }
    }

    fn apply_frequency_settings(&mut self, manager: &FrequencyManager) {
        if let Some(states) = manager.dictionary_states() {
            for (name, state) in states {
                let setting =
                    self.settings_data.frequency_weights.get(&name).cloned().unwrap_or_else(|| {
                        FrequencyDictionarySetting { weight: state.weight, enabled: state.enabled }
                    });
                let weight = setting.weight.max(0.1);
                if let Err(err) = manager.set_dictionary_state(&name, weight, setting.enabled) {
                    eprintln!("Failed to update dictionary state '{}': {}", name, err);
                }
            }
        }

        self.table_state.sync_frequency_states(Some(manager));
        self.table_state.mark_dirty();
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

    fn handle_file_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input_mut(|i| mem::take(&mut i.raw.dropped_files));
        if dropped.is_empty() {
            return;
        }

        if let Some(path) = dropped.iter().filter_map(|f| f.path.as_deref()).find(|p| {
            let ft = SourceFileType::from_extension(&p.to_string_lossy());
            ft.is_supported()
        }) {
            let source_file =
                FileModal::create_source_file_from_path_and_metadata(path, None, None);

            self.modals.file.close();
            self.process_source_file(source_file);
        }
    }

    fn draw_file_drop_overlay(&self, ctx: &egui::Context) {
        let hovering_any = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !hovering_any {
            return;
        }

        let any_valid_hovered = ctx.input(|i| {
            i.raw.hovered_files.iter().filter_map(|f| f.path.as_deref()).any(|p| {
                let ft = SourceFileType::from_extension(&p.to_string_lossy());
                ft.is_supported()
            })
        });
        if !any_valid_hovered {
            return;
        }

        let size = egui::vec2(300.0, 120.0);

        egui::Modal::new(Id::new("file_drop_overlay")).show(ctx, |ui| {
            ui.set_max_size(size);
            ui.set_min_size(size);

            ui.centered_and_justified(|ui| {
                ui.heading("📥  Drop to open");
            });
        });
    }
}
