use std::path::Path;

use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFileEntry {
    pub file_path: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>, // e.g. the EPUB chapter selection last mined
    pub creator: Option<String>,
    pub last_opened: chrono::DateTime<chrono::Utc>,
    pub file_size: Option<u64>,
    pub term_count: Option<usize>,
}

impl RecentFileEntry {
    pub fn new(
        file_path: String,
        title: String,
        subtitle: Option<String>,
        creator: Option<String>,
        term_count: usize,
    ) -> Self {
        let file_size = std::fs::metadata(&file_path).map(|metadata| metadata.len()).ok();

        Self {
            file_path,
            title,
            subtitle,
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

impl Default for RecentFileEntry {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            title: String::new(),
            subtitle: None,
            creator: None,
            last_opened: chrono::Utc::now(),
            file_size: None,
            term_count: None,
        }
    }
}
