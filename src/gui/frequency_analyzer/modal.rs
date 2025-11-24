use std::path::PathBuf;

use eframe::egui::{
    self,
    Modal,
};

use crate::{
    core::tasks::types::CancellableTask,
    gui::LanguageTools,
    tools::analysis::{
        AnalysisOptions,
        AnalysisProgress,
        AnalysisState,
        CorpusBalancer,
        ExportOptions,
        FileTreeBuilder,
        FileTreeNode,
        FileTreeState,
        FrequencyAnalysisResult,
        TermEntry,
        TreeNodeId,
    },
};

const MODAL_WIDTH: f32 = 700.0;
const MODAL_MAX_HEIGHT: f32 = 500.0;
const SCROLL_AREA_HEIGHT: f32 = 350.0;
const FILE_TREE_HEIGHT: f32 = 160.0;
const FILE_TREE_WIDTH: f32 = 500.0;

const SMALL_SPACING: f32 = 4.0;
const MEDIUM_SPACING: f32 = 6.0;
const BUTTON_SPACING: f32 = 6.0;
const LARGE_SPACING: f32 = 10.0;

const RESULTS_DISPLAY_LIMIT: usize = 250;

const COLOR_SUCCESS: egui::Color32 = egui::Color32::from_rgb(0, 200, 0);
const COLOR_WARNING: egui::Color32 = egui::Color32::from_rgb(255, 165, 0);
const COLOR_ERROR: egui::Color32 = egui::Color32::from_rgb(255, 100, 100);

fn determine_message_color(msg: &str) -> egui::Color32 {
    if msg.starts_with("✓") || msg.starts_with("Success") {
        COLOR_SUCCESS
    } else if msg.starts_with("⚠") || msg.starts_with("Warning") {
        COLOR_WARNING
    } else {
        COLOR_ERROR
    }
}

enum SelectionMode {
    Files,
    Folder,
}

pub struct FrequencyAnalyzerModal {
    open: bool,
    selected_files: Vec<PathBuf>,
    file_tree: FileTreeState,
    tree_view_state: egui_ltreeview::TreeViewState<TreeNodeId>,
    state: AnalysisState,
    result: Option<FrequencyAnalysisResult>,
    sorted_entries: Vec<TermEntry>,
    export_options: ExportOptions,
    analysis_options: AnalysisOptions,
    progress: AnalysisProgress,
}

impl Default for FrequencyAnalyzerModal {
    fn default() -> Self {
        Self {
            open: false,
            selected_files: Vec::new(),
            file_tree: FileTreeState::default(),
            tree_view_state: egui_ltreeview::TreeViewState::default(),
            state: AnalysisState::SelectingFiles,
            result: None,
            sorted_entries: Vec::new(),
            export_options: ExportOptions::default(),
            analysis_options: AnalysisOptions::default(),
            progress: AnalysisProgress::default(),
        }
    }
}

impl FrequencyAnalyzerModal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_modal(&mut self) {
        self.open = true;

        if !matches!(
            self.state,
            AnalysisState::Analyzing
                | AnalysisState::ShowingResults
                | AnalysisState::Exporting
                | AnalysisState::ExportComplete(_)
        ) {
            self.selected_files.clear();
            self.file_tree = FileTreeState::default();
            self.state = AnalysisState::SelectingFiles;
            self.result = None;
            self.sorted_entries.clear();
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn add_files(&mut self, mode: SelectionMode) {
        use std::collections::HashSet;

        use crate::tools::analysis::utils::find_supported_files_recursive;

        match mode {
            SelectionMode::Files => {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("Subtitle/Text files", &["srt", "ass", "ssa", "txt"])
                    .pick_files()
                {
                    let existing: HashSet<_> = self.selected_files.iter().collect();
                    let new_files: Vec<_> =
                        files.into_iter().filter(|f| !existing.contains(f)).collect();

                    if !new_files.is_empty() {
                        self.selected_files.extend(new_files);
                        self.build_file_tree();
                    }
                }
            }
            SelectionMode::Folder => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let found_files = find_supported_files_recursive(&folder);

                    if !found_files.is_empty() {
                        let existing: HashSet<_> = self.selected_files.iter().collect();
                        let new_files: Vec<_> =
                            found_files.into_iter().filter(|f| !existing.contains(f)).collect();

                        if !new_files.is_empty() {
                            self.selected_files.extend(new_files);
                            self.build_file_tree();
                        }
                    }
                }
            }
        }
    }

    fn analyze(
        &mut self,
        language_tools: &LanguageTools,
        task_manager: &crate::core::tasks::TaskManager,
    ) {
        if self.selected_files.is_empty() {
            self.state = AnalysisState::Error("No files selected".to_string());
            return;
        }

        let files_to_analyze = if self.analysis_options.balance_corpus {
            CorpusBalancer::new(self.selected_files.clone()).balance()
        } else {
            self.selected_files.clone()
        };

        let total_bytes: u64 = files_to_analyze
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();

        self.progress = AnalysisProgress {
            total_files: files_to_analyze.len(),
            total_bytes,
            start_time: Some(std::time::Instant::now()),
            ..Default::default()
        };
        self.state = AnalysisState::Analyzing;

        task_manager.analyze_frequency(files_to_analyze, language_tools.clone());
    }

    pub fn handle_analysis_progress(
        &mut self,
        current_file: usize,
        message: String,
        file_size: u64,
    ) {
        self.progress.current_file = current_file;
        self.progress.message = message;
        self.progress.bytes_processed += file_size;
    }

    pub fn handle_analysis_complete(&mut self, result: Result<FrequencyAnalysisResult, String>) {
        // Only process completion if still in Analyzing state (not cancelled)
        if !matches!(self.state, AnalysisState::Analyzing) {
            return;
        }

        match result {
            Ok(result) => {
                // Create sorted entries for display
                let mut entries: Vec<TermEntry> = result
                    .frequencies
                    .iter()
                    .map(|((term, _reading), &frequency)| TermEntry {
                        term: term.clone(),
                        frequency,
                    })
                    .collect();

                // Sort by frequency descending
                entries.sort_by(|a, b| b.frequency.cmp(&a.frequency));

                self.sorted_entries = entries;
                self.result = Some(result);
                self.state = AnalysisState::ShowingResults;
            }
            Err(e) => {
                self.state = AnalysisState::Error(format!("Analysis failed: {}", e));
            }
        }
    }

    fn build_file_tree(&mut self) {
        self.file_tree = FileTreeBuilder::build_tree(&self.selected_files);

        // Collapse all newly added directories
        if let Some(ref root) = self.file_tree.root {
            let dir_ids = Self::collect_all_directory_ids(root, String::new());
            for dir_id in dir_ids {
                if self.tree_view_state.is_open(&dir_id).is_none() {
                    self.tree_view_state.set_openness(dir_id, false);
                }
            }
        }
    }

    fn render_tree_with_ltreeview(&mut self, ui: &mut egui::Ui) {
        if let Some(root) = self.file_tree.root.clone() {
            use egui_ltreeview::TreeView;

            let tree_id = ui.make_persistent_id("freq_analyzer_tree");

            TreeView::new(tree_id).allow_multi_selection(false).show_state(
                ui,
                &mut self.tree_view_state,
                |builder| {
                    for child in &root.children {
                        Self::build_tree_node(builder, child, String::new());
                    }
                },
            );
        }
    }

    fn collect_all_directory_ids(node: &FileTreeNode, parent_path: String) -> Vec<TreeNodeId> {
        let mut dir_ids = Vec::new();

        if node.path.is_none() {
            let dir_path = if parent_path.is_empty() {
                node.name.clone()
            } else {
                format!("{}/{}", parent_path, node.name)
            };

            dir_ids.push(TreeNodeId::Directory(dir_path.clone()));

            for child in &node.children {
                dir_ids.extend(Self::collect_all_directory_ids(child, dir_path.clone()));
            }
        } else {
        }

        dir_ids
    }

    fn build_tree_node(
        builder: &mut egui_ltreeview::TreeViewBuilder<TreeNodeId>,
        node: &FileTreeNode,
        parent_path: String,
    ) {
        if let Some(path) = &node.path {
            let node_id = TreeNodeId::File(path.clone());
            let label = format!("{}", node.name);
            builder.leaf(node_id, label);
        } else {
            let dir_path = if parent_path.is_empty() {
                node.name.clone()
            } else {
                format!("{}/{}", parent_path, node.name)
            };

            let file_count = node.count_files();
            let label = format!("{} ({} files)", node.name, file_count);
            let node_id = TreeNodeId::Directory(dir_path.clone());

            builder.dir(node_id, label);

            for child in &node.children {
                Self::build_tree_node(builder, child, dir_path.clone());
            }

            builder.close_dir();
        }
    }

    fn find_node_by_name<'a>(
        &self,
        node: &'a FileTreeNode,
        name: &str,
    ) -> Option<&'a FileTreeNode> {
        if node.path.is_none() && node.name == name {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = self.find_node_by_name(child, name) {
                return Some(found);
            }
        }
        None
    }

    fn collect_paths_from_node(&self, node: &FileTreeNode) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        self.collect_paths_recursive(node, &mut paths);
        paths
    }

    fn collect_paths_recursive(&self, node: &FileTreeNode, out: &mut Vec<PathBuf>) {
        if let Some(ref path) = node.path {
            out.push(path.clone());
        }
        for child in &node.children {
            self.collect_paths_recursive(child, out);
        }
    }

    fn export(&mut self, task_manager: &crate::core::tasks::TaskManager) {
        if let Some(result) = &self.result {
            if let Some(output_dir) = rfd::FileDialog::new().pick_folder() {
                self.state = AnalysisState::Exporting;

                task_manager.export_frequency(
                    result.clone(),
                    output_dir,
                    self.export_options.dict_name.clone(),
                    self.export_options.dict_author.clone(),
                    self.export_options.dict_url.clone(),
                    self.export_options.dict_description.clone(),
                    self.export_options.revision_prefix.clone(),
                    self.export_options.export_yomitan,
                    self.export_options.export_csv,
                    self.export_options.pretty_json,
                    self.export_options.exclude_hapax,
                );
            }
        }
    }

    pub fn handle_export_complete(&mut self, result: Result<String, String>) {
        match result {
            Ok(msg) => self.state = AnalysisState::ExportComplete(msg),
            Err(err) => self.state = AnalysisState::Error(err),
        }
    }

    fn ui_file_selection_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step 1 · Select files");
        ui.add_space(SMALL_SPACING);

        // Buttons row
        ui.horizontal(|ui| {
            if ui.button("Add Files…").clicked() {
                self.add_files(SelectionMode::Files);
            }
            if ui.button("Add Folder…").clicked() {
                self.add_files(SelectionMode::Folder);
            }
        });

        ui.add_space(SMALL_SPACING);

        // Selection controls row
        ui.horizontal(|ui| {
            if ui.small_button("Clear all").clicked() {
                self.selected_files.clear();
                self.file_tree = FileTreeState::default();
            }
            ui.label(format!("{} files selected", self.selected_files.len()));
        });

        ui.add_space(MEDIUM_SPACING);

        if !self.selected_files.is_empty() {
            egui::ScrollArea::vertical()
                .id_salt("file_tree_scroll")
                .max_height(FILE_TREE_HEIGHT)
                .max_width(FILE_TREE_WIDTH)
                .show(ui, |ui| {
                    self.render_tree_with_ltreeview(ui);
                });

            ui.add_space(SMALL_SPACING);

            ui.horizontal(|ui| {
                let has_selection = !self.tree_view_state.selected().is_empty();
                if ui.add_enabled(has_selection, egui::Button::new("Remove Selected")).clicked() {
                    if let Some(selected) = self.tree_view_state.selected().first() {
                        match selected {
                            TreeNodeId::File(path) => {
                                self.selected_files.retain(|p| p != path);
                                self.build_file_tree();
                            }
                            TreeNodeId::Directory(dir_name) => {
                                if let Some(root) = &self.file_tree.root {
                                    if let Some(node) = self.find_node_by_name(root, dir_name) {
                                        let paths_to_remove = self.collect_paths_from_node(node);
                                        self.selected_files
                                            .retain(|p| !paths_to_remove.contains(p));
                                        self.build_file_tree();
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    fn ui_analysis_controls(
        &mut self,
        ui: &mut egui::Ui,
        language_tools: Option<&LanguageTools>,
        task_manager: &crate::core::tasks::TaskManager,
    ) {
        ui.add_space(LARGE_SPACING);
        ui.heading("Step 2 · Analyze");
        ui.add_space(SMALL_SPACING);

        if self.selected_files.is_empty() {
            ui.weak("Select at least one file on the left.");
            return;
        }

        ui.checkbox(&mut self.analysis_options.balance_corpus, "Balance corpus by source")
            .on_hover_text("Uses trimmed mean (10% trimming) to calculate balanced sample sizes.");

        ui.add_space(MEDIUM_SPACING);

        let can_analyze = language_tools.is_some() && !self.selected_files.is_empty();

        if language_tools.is_none() {
            ui.weak("Language tools not initialized");
        }

        if ui.add_enabled(can_analyze, egui::Button::new("Analyze files")).clicked() {
            self.analyze(language_tools.unwrap(), task_manager);
        }
    }

    fn ui_results_and_export_section(
        &mut self,
        ui: &mut egui::Ui,
        task_manager: &crate::core::tasks::TaskManager,
    ) {
        ui.horizontal(|ui| {
            ui.heading("Step 3 · Results & export");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("← Back to File Selection").clicked() {
                    self.state = AnalysisState::SelectingFiles;
                }
            });
        });
        ui.add_space(SMALL_SPACING);

        if let Some(ref result) = self.result {
            let summary = format!(
                "Total: {} terms | Unique: {} terms",
                result.total_terms, result.unique_terms
            );

            if result.skipped_files.is_empty() {
                ui.label(summary);
            } else {
                let skipped_details = result
                    .skipped_files
                    .iter()
                    .map(|(name, err)| format!("• {}: {}", name, err))
                    .collect::<Vec<_>>()
                    .join("\n");
                ui.label(summary).on_hover_text(format!(
                    "⚠ Skipped {} file(s):\n{}",
                    result.skipped_files.len(),
                    skipped_details
                ));
            }
        }

        ui.add_space(MEDIUM_SPACING);

        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                use crate::gui::frequency_analyzer::ResultsTableWidget;
                ResultsTableWidget::show(
                    ui,
                    &self.sorted_entries,
                    &mut self.analysis_options.show_top,
                    RESULTS_DISPLAY_LIMIT,
                );
            });

            columns[1].vertical(|ui| {
                use crate::gui::frequency_analyzer::ExportFormWidget;
                if ExportFormWidget::show(ui, &mut self.export_options) {
                    self.export(task_manager);
                }
            });
        });
    }

    fn ui_exporting_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Exporting...");
        });
    }

    fn ui_analyzing_section(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        task_manager: &crate::core::tasks::TaskManager,
    ) {
        ui.heading("Step 3 · Analyzing");
        ui.add_space(LARGE_SPACING);

        use crate::gui::frequency_analyzer::AnalysisProgressWidget;
        if AnalysisProgressWidget::show(ui, ctx, &mut self.progress) {
            task_manager.cancel_task(CancellableTask::FrequencyAnalysis);
            self.state = AnalysisState::SelectingFiles;
            self.result = None;
            self.sorted_entries.clear();
        }
    }

    fn ui_message_section(&mut self, ui: &mut egui::Ui, msg: &str, is_success: bool) {
        let color = determine_message_color(msg);

        ui.colored_label(color, msg);
        ui.add_space(LARGE_SPACING);

        if is_success {
            if ui.button("← Back to Results").clicked() {
                self.state = AnalysisState::ShowingResults;
            }
        } else {
            if ui.button("Start New Analysis").clicked() {
                self.state = AnalysisState::SelectingFiles;
                self.result = None;
                self.sorted_entries.clear();
            }
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        language_tools: Option<&LanguageTools>,
        task_manager: &crate::core::tasks::TaskManager,
    ) {
        if !self.open {
            return;
        }

        let modal = Modal::new(egui::Id::new("frequency_analyzer_modal")).show(ctx, |ui| {
            // Constrain the modal width
            ui.set_max_width(MODAL_WIDTH);
            ui.set_max_height(MODAL_MAX_HEIGHT);

            // State-based layout inside scrollable area
            egui::ScrollArea::vertical()
                .max_height(SCROLL_AREA_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.heading("Frequency Analyzer");
                    ui.add_space(BUTTON_SPACING);

                    match &self.state {
                        AnalysisState::SelectingFiles => {
                            // File Selection & Analysis Setup
                            self.ui_file_selection_section(ui);
                            self.ui_analysis_controls(ui, language_tools, task_manager);
                        }
                        AnalysisState::Analyzing => {
                            self.ui_analyzing_section(ui, ctx, task_manager);
                        }
                        AnalysisState::ShowingResults => {
                            self.ui_results_and_export_section(ui, task_manager);
                        }
                        AnalysisState::Exporting => {
                            self.ui_exporting_section(ui);
                        }
                        AnalysisState::ExportComplete(msg) => {
                            let msg_clone = msg.clone();
                            self.ui_message_section(ui, &msg_clone, true);
                        }
                        AnalysisState::Error(msg) => {
                            let msg_clone = msg.clone();
                            self.ui_message_section(ui, &msg_clone, false);
                        }
                    }
                });

            ui.add_space(BUTTON_SPACING);
            ui.separator();
            ui.add_space(SMALL_SPACING);

            // Close button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("Close").clicked() {
                    ui.close();
                }
            });
        });

        if modal.should_close() {
            self.open = false;
        }
    }
}
