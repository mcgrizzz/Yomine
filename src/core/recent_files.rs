use std::{
    collections::VecDeque,
    path::Path,
};

use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFileEntry {
    pub file_path: String,
    pub title: String,
    pub creator: Option<String>,
    pub last_opened: chrono::DateTime<chrono::Utc>,
    pub file_size: Option<u64>,
    pub term_count: Option<usize>,
}

impl RecentFileEntry {
    pub fn new(
        file_path: String,
        title: String,
        creator: Option<String>,
        term_count: usize,
    ) -> Self {
        let file_size = std::fs::metadata(&file_path).map(|metadata| metadata.len()).ok();

        Self {
            file_path,
            title,
            creator,
            last_opened: chrono::Utc::now(),
            file_size,
            term_count: Some(term_count),
        }
    }

    pub fn file_exists(&self) -> bool {
        Path::new(&self.file_path).exists()
    }

    pub fn format_last_opened(&self) -> String {
        let local_time = self.last_opened.with_timezone(&chrono::Local);
        local_time.format("%Y-%m-%d %H:%M").to_string()
    }

    pub fn format_file_size(&self) -> String {
        match self.file_size {
            Some(size) => {
                if size < 1024 {
                    format!("{} B", size)
                } else if size < 1024 * 1024 {
                    format!("{:.1} KB", size as f64 / 1024.0)
                } else {
                    format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
                }
            }
            None => "Unknown".to_string(),
        }
    }

    pub fn format_term_count(&self) -> String {
        match self.term_count {
            Some(count) => {
                if count == 1 {
                    "1 term".to_string()
                } else {
                    format!("{} terms", count)
                }
            }
            None => "Unknown terms".to_string(),
        }
    }

    pub fn get_filename(&self) -> String {
        Path::new(&self.file_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown")
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFiles {
    files: VecDeque<RecentFileEntry>,
    max_entries: usize,
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new(10) // Default to 10 recent files
    }
}

impl RecentFiles {
    pub fn new(max_entries: usize) -> Self {
        Self { files: VecDeque::new(), max_entries }
    }

    pub fn add_file(
        &mut self,
        file_path: String,
        title: String,
        creator: Option<String>,
        term_count: usize,
    ) {
        self.files.retain(|entry| entry.file_path != file_path);

        let new_entry = RecentFileEntry::new(file_path, title, creator, term_count);
        self.files.push_front(new_entry);

        while self.files.len() > self.max_entries {
            self.files.pop_back();
        }
    }

    pub fn get_files(&self) -> &VecDeque<RecentFileEntry> {
        &self.files
    }

    pub fn get_valid_files(&self) -> Vec<&RecentFileEntry> {
        self.files.iter().filter(|entry| entry.file_exists()).collect()
    }

    pub fn remove_file(&mut self, file_path: &str) {
        self.files.retain(|entry| entry.file_path != file_path);
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn cleanup_missing_files(&mut self) {
        self.files.retain(|entry| entry.file_exists());
    }
}

impl Default for RecentFileEntry {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            title: String::new(),
            creator: None,
            last_opened: chrono::Utc::now(),
            file_size: None,
            term_count: None,
        }
    }
}
