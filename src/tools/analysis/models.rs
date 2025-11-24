use std::{
    collections::HashSet,
    path::PathBuf,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum TreeNodeId {
    File(PathBuf),
    Directory(String),
}

#[derive(Debug, Clone)]
pub struct FileTreeNode {
    pub name: String,
    pub path: Option<PathBuf>,
    pub children: Vec<FileTreeNode>,
}

impl FileTreeNode {
    pub fn count_files(&self) -> usize {
        let mut count = 0;
        for child in &self.children {
            if child.path.is_some() {
                count += 1;
            } else {
                count += child.count_files();
            }
        }
        count
    }
}

#[derive(Debug, Clone)]
pub struct FileTreeState {
    pub root: Option<FileTreeNode>,
    pub selected: HashSet<PathBuf>,
}

impl Default for FileTreeState {
    fn default() -> Self {
        Self { root: None, selected: HashSet::new() }
    }
}

#[derive(Debug, Clone)]
pub struct TermEntry {
    pub term: String,
    pub frequency: u32,
}

#[derive(Debug, Clone)]
pub enum AnalysisState {
    SelectingFiles,
    Analyzing,
    ShowingResults,
    Exporting,
    ExportComplete(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub dict_name: String,
    pub dict_author: String,
    pub dict_url: String,
    pub dict_description: String,
    pub revision_prefix: String,
    pub export_yomitan: bool,
    pub export_csv: bool,
    pub pretty_json: bool,
    pub exclude_hapax: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            dict_name: String::from("custom_frequency"),
            dict_author: String::new(),
            dict_url: String::new(),
            dict_description: String::new(),
            revision_prefix: String::new(),
            export_yomitan: true,
            export_csv: false,
            pretty_json: false,
            exclude_hapax: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    pub balance_corpus: bool,
    pub show_top: bool,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self { balance_corpus: false, show_top: true }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisProgress {
    pub total_files: usize,
    pub current_file: usize,
    pub message: String,
    pub start_time: Option<std::time::Instant>,
    pub total_bytes: u64,
    pub bytes_processed: u64,
    pub smoothed_estimate: Option<f32>,
}

impl AnalysisProgress {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn calculate_fraction(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        self.current_file as f32 / self.total_files as f32
    }

    pub fn calculate_time_estimate(&mut self, elapsed: f32) -> Option<f32> {
        if self.total_bytes == 0 || self.bytes_processed == 0 {
            return None;
        }

        let bytes_remaining = self.total_bytes.saturating_sub(self.bytes_processed);
        let bytes_per_second = self.bytes_processed as f32 / elapsed;
        let raw_estimate = bytes_remaining as f32 / bytes_per_second;

        // Apply exponential smoothing (alpha = 0.3)
        const ALPHA: f32 = 0.3;
        let smoothed = match self.smoothed_estimate {
            Some(prev) => ALPHA * raw_estimate + (1.0 - ALPHA) * prev,
            None => raw_estimate,
        };

        self.smoothed_estimate = Some(smoothed);
        Some(smoothed)
    }
}
