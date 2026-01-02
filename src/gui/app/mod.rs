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
    top_bar::{
        TopBar,
        TopBarAction,
    },
    websocket_manager::WebSocketManager,
};
use crate::{
    anki::FieldMapping,
    core::{
        media_server::MediaServerStream,
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
    // pub last_mpv_connected: bool,
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
            // last_mpv_connected: false,
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
        
        // let mpv_connected = self.player.mpv.is_connected();
        // if mpv_connected && !self.last_mpv_connected {
        //     println!("[MPV] Connection established, automatically checking for subtitles...");
        //     self.load_from_mpv();
        // }
        // self.last_mpv_connected = mpv_connected;

        self.update_anki_status();
        self.handle_file_drops(ctx);
        self.draw_file_drop_overlay(ctx);

        let ignore_list_ref = self.language_tools.as_ref().map(|lt| &lt.ignore_list);
        let frequency_manager =
            self.language_tools.as_ref().map(|lt| lt.frequency_manager.as_ref());

        let can_refresh = self.file_data.as_ref().map_or(false, |fd| !fd.original_terms.is_empty())
            && self.language_tools.is_some();

        if let Some(action) = TopBar::show(
            ctx,
            &mut self.modals.file,
            &mut self.modals.anki_settings,
            &mut self.modals.websocket_settings,
            &mut self.modals.ignore_list,
            &mut self.modals.frequency_weights,
            &mut self.modals.pos_filters,
            &mut self.modals.frequency_analyzer,
            &mut self.modals.setup_checklist,
            &mut self.settings_data,
            &self.player.ws,
            self.player.mpv.is_connected(),
            self.anki_connected,
            ignore_list_ref,
            &self.task_manager,
            can_refresh,
            &self.table_state,
            frequency_manager,
        ) {
            match action {
                TopBarAction::LoadFromMpv => self.load_from_mpv(),
            }
        }

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

        if let Some(track) = self.modals.subtitle_select.show(ctx) {
            self.load_subtitle_resource(&track.download_url);
        }

        term_table(ctx, self);
        self.message_overlay.show(ctx, &self.theme);
        self.modals.error.show(ctx);

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
                    frequency_utils::load_frequency_dictionaries(&self.task_manager);
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
            TaskResult::FrequencyDictionariesReloaded(result) => {
                self.message_overlay.clear_message();

                match result {
                    Ok(new_freq_manager) => {
                        if let Some(language_tools) = &mut self.language_tools {
                            language_tools.frequency_manager = new_freq_manager.clone();

                            self.apply_frequency_settings(new_freq_manager.as_ref());

                            self.table_state.mark_dirty();
                            if let Some(file_data) = &self.file_data {
                                self.table_state.ensure_indices(
                                    &file_data.terms,
                                    &file_data.sentences,
                                    Some(new_freq_manager.as_ref()),
                                );
                            }

                            println!(
                                "Successfully reloaded {} frequency dictionaries!",
                                new_freq_manager.get_dictionary_names().len()
                            );
                        }
                    }
                    Err(e) => {
                        self.modals.error.show_error(
                            "Reload Error".to_string(),
                            "Failed to reload frequency dictionaries",
                            Some(&e),
                        );
                    }
                }
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

    fn load_from_mpv(&mut self) {
        if !self.player.mpv.is_connected() {
            println!("[MPV Load] MPV is not connected.");
            self.modals.error.show_error(
                "MPV Error".to_string(),
                "MPV is not connected".to_string().as_str(),
                None::<String>,
            );
            return;
        }

        println!("[MPV Load] Starting load process...");

        let is_japanese = |lang: &str, title: &str| -> bool {
            let l = lang.to_lowercase();
            let t = title.to_lowercase();
            l == "jpn" || l == "ja" || l == "jp" || 
            t.contains("japanese") || t.contains("日本語") || t.contains("jp") || t.contains("ja")
        };

        // 1. Try to find an active external subtitle track
        match self.player.mpv.get_property("track-list") {
            Ok(track_list) => {
                if let Some(tracks) = track_list.as_array() {
                    for (index, track) in tracks.iter().enumerate() {
                        // Check if it's a selected subtitle track
                        if track["type"] == "sub" && track["selected"] == true {
                            let lang = track["lang"].as_str().unwrap_or("");
                            let title = track["title"].as_str().unwrap_or("");
                            
                            if !is_japanese(lang, title) {
                                println!("[MPV Load] Selected track at index {} is not Japanese ({}/{}). Skipping auto-load.", index, lang, title);
                                continue;
                            }

                            println!("[MPV Load] Found selected Japanese subtitle track at index {}.", index);
                            // Check for external-filename first
                            if let Some(filename) = track["external-filename"].as_str() {
                                println!("[MPV Load] Found 'external-filename': {}", filename);
                                self.load_subtitle_resource(filename);
                                return;
                            }
                            
                            // Fallback to 'filename' if it's marked as external
                            if track["external"] == true {
                                 if let Some(filename) = track["filename"].as_str() {
                                    println!("[MPV Load] Found 'filename' (marked external): {}", filename);
                                    self.load_subtitle_resource(filename);
                                    return;
                                 }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("[MPV Load] Failed to get track-list: {}", e);
            }
        }

        // 2. Fallback: Check "path" property (video/file path)
        match self.player.mpv.get_property_string("path") {
            Ok(path_str) => {
                println!("[MPV Load] Retrieved path: {}", path_str);
                let path = std::path::PathBuf::from(&path_str);
                
                // If the path itself is a subtitle file
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                if ["srt", "ass", "ssa", "txt"].contains(&ext.as_str()) {
                     println!("[MPV Load] Path identified as subtitle file (extension: {}).", ext);
                     self.load_subtitle_resource(&path_str);
                     return;
                }

                // Check for Emby/Jellyfin stream
                if path_str.starts_with("http://") || path_str.starts_with("https://") {
                     if let Some(stream) = MediaServerStream::try_parse(&path_str) {
                         println!("[MPV Load] Detected Media Server Stream: {:?}", stream);
                         match stream.fetch_subtitles() {
                             Ok(tracks) => {
                                 // Filter for Japanese tracks
                                 let jp_tracks: Vec<_> = tracks.into_iter().filter(|t| {
                                     is_japanese(&t.language, &t.title)
                                 }).collect();

                                 println!("[MPV Load] Found {} Japanese subtitle tracks via API.", jp_tracks.len());
                                 
                                 if jp_tracks.len() == 1 {
                                     println!("[MPV Load] Only one Japanese track found. Loading automatically.");
                                     self.load_subtitle_resource(&jp_tracks[0].download_url);
                                     return;
                                 } else if jp_tracks.len() > 1 {
                                     println!("[MPV Load] Multiple Japanese tracks found. Opening selector.");
                                     self.modals.subtitle_select.open(jp_tracks);
                                     return;
                                 } else {
                                     println!("[MPV Load] No Japanese subtitles found via Media Server API.");
                                 }
                             },
                             Err(e) => {
                                 println!("[MPV Load] Failed to fetch subtitles from Media Server: {}", e);
                                 self.modals.error.show_error(
                                    "API Error".to_string(),
                                    "Failed to fetch subtitles from Media Server".to_string().as_str(),
                                    Some(&e),
                                );
                             }
                         }
                     }
                }

                // If it's a local file, try to find a sibling subtitle
                if !path_str.starts_with("http://") && !path_str.starts_with("https://") {
                    // Try to resolve absolute path if it's relative
                    let path = if path.is_absolute() || path_str.starts_with(r"\\") || (path_str.starts_with(r"\") && !path_str.starts_with(r"\\")) {
                        // Absolute path or UNC path (including single backslash variant)
                        path
                    } else {
                        // Try to get MPV working directory
                        match self.player.mpv.get_property_string("working-directory") {
                            Ok(wd) => {
                                println!("[MPV Load] Working directory: {}", wd);
                                std::path::Path::new(&wd).join(path)
                            },
                            Err(e) => {
                                println!("[MPV Load] Failed to get working-directory: {}", e);
                                path // Fallback to as-is
                            }
                        }
                    };
                    
                    println!("[MPV Load] Resolved absolute path: {}", path.display());

                    if path.exists() {
                        // Attempt to find subtitle with same name
                        println!("[MPV Load] File exists. Checking for sibling subtitles...");
                        let mut sub_found = false;
                        for sub_ext in ["srt", "ass", "ssa", "txt"] {
                            let mut sub_path = path.clone();
                            sub_path.set_extension(sub_ext);
                            if sub_path.exists() {
                                println!("[MPV Load] Found sibling subtitle: {}", sub_path.display());
                                self.load_subtitle_resource(&sub_path.to_string_lossy());
                                sub_found = true;
                                break;
                            }
                        }

                        if !sub_found {
                             println!("[MPV Load] No sibling subtitle found.");
                             self.modals.error.show_error(
                                "File Load Error".to_string(),
                                "No external subtitle track found. Tried to find local sibling file but failed.".to_string().as_str(),
                                Some(&format!("Video Path: {}", path.display())),
                            );
                        }
                    } else {
                         println!("[MPV Load] Resolved path does not exist or is not accessible.");
                         self.modals.error.show_error(
                            "File Load Error".to_string(),
                            "MPV reported path does not exist or is not accessible. Check network connectivity if using UNC paths.".to_string().as_str(),
                            Some(&path_str),
                        );
                    }
                } else {
                     println!("[MPV Load] Path is a URL, and no external track found. Cannot infer subtitle.");
                     self.modals.error.show_error(
                        "File Load Error".to_string(),
                        "No external subtitle track found for this stream.".to_string().as_str(),
                        Some(&path_str),
                    );
                }
            }
            Err(e) => {
                println!("[MPV Load] Failed to get path: {}", e);
                self.modals.error.show_error(
                    "MPV Error".to_string(),
                    "Failed to get path from MPV".to_string().as_str(),
                    Some(&e.to_string()),
                );
            }
        }
    }

    fn load_subtitle_resource(&mut self, path_str: &str) {
        println!("[MPV Load] Loading resource: {}", path_str);
        
        // Normalize UNC paths (convert forward slashes to backslashes on Windows)
        // Also fix single leading backslash that MPV sometimes reports
        let normalized_path = if path_str.starts_with(r"\\") || path_str.starts_with("//") {
            path_str.replace("/", r"\")
        } else if path_str.starts_with(r"\") && !path_str.starts_with(r"\\") {
            // Single leading backslash - likely a UNC path missing one backslash
            format!(r"\{}", path_str)
        } else {
            path_str.to_string()
        };
        
        if normalized_path.starts_with("http://") || normalized_path.starts_with("https://") {
             // It's a URL, download it
             self.message_overlay.set_message("Downloading subtitle...".to_string());
             
             match reqwest::blocking::get(path_str) {
                 Ok(mut response) => {
                     // Try to guess extension
                     // Strip query parameters for extension guessing
                     let clean_path = path_str.split('?').next().unwrap_or(path_str);
                     let url_path = std::path::Path::new(clean_path);
                     let ext = url_path.extension().and_then(|e| e.to_str()).unwrap_or("srt");
                     
                     println!("[MPV Load] Guessed extension '{}' from URL '{}'", ext, clean_path);

                     // Validate extension, default to srt if unknown
                     let safe_ext = if ["srt", "ass", "ssa", "txt"].contains(&ext.to_lowercase().as_str()) { 
                         ext 
                     } else { 
                         // Check content-type header if available? 
                         // For now, just default to srt as it's most common for streams
                         "srt" 
                     };

                     let temp_dir = std::env::temp_dir();
                     let temp_filename = format!("yomine_mpv_{}.{}", uuid::Uuid::new_v4(), safe_ext);
                     let temp_file_path = temp_dir.join(temp_filename);
                     
                     println!("[MPV Load] Downloading to temp file: {}", temp_file_path.display());

                     match std::fs::File::create(&temp_file_path) {
                         Ok(mut file) => {
                             if let Err(e) = response.copy_to(&mut file) {
                                  println!("[MPV Load] Download failed: {}", e);
                                  self.message_overlay.clear_message();
                                  self.modals.error.show_error("Download Error".to_string(), "Failed to save subtitle".to_string().as_str(), Some(&e.to_string()));
                                  return;
                             }
                             println!("[MPV Load] Download successful.");
                             self.message_overlay.clear_message();
                             
                             let source_file = FileModal::create_source_file_from_path_and_metadata(
                                 &temp_file_path,
                                 None,
                                 None
                             );
                             self.process_source_file(source_file);
                         }
                         Err(e) => {
                             println!("[MPV Load] Failed to create temp file: {}", e);
                             self.message_overlay.clear_message();
                             self.modals.error.show_error("IO Error".to_string(), "Failed to create temp file".to_string().as_str(), Some(&e.to_string()));
                         }
                     }
                 }
                 Err(e) => {
                     println!("[MPV Load] Network request failed: {}", e);
                     self.message_overlay.clear_message();
                     self.modals.error.show_error("Network Error".to_string(), "Failed to download subtitle".to_string().as_str(), Some(&e.to_string()));
                 }
             }
        } else {
            // Local file or UNC path
            let path = std::path::PathBuf::from(&normalized_path);
            if path.exists() {
                 println!("[MPV Load] File exists (local or network). Processing...");
                 let source_file = FileModal::create_source_file_from_path_and_metadata(&path, None, None);
                 self.process_source_file(source_file);
            } else {
                 println!("[MPV Load] File does not exist or is not accessible: {}", path.display());
                 let error_detail = if normalized_path.starts_with(r"\\") {
                     format!("Network path not accessible: {}\nCheck network connectivity and permissions.", normalized_path)
                 } else {
                     format!("Local path not found: {}", normalized_path)
                 };
                 self.modals.error.show_error(
                    "File Error".to_string(), 
                    "Subtitle file reported by MPV does not exist or is not accessible.".to_string().as_str(), 
                    Some(&error_detail)
                );
            }
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
