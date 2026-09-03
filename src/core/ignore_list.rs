use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
};

use serde::{
    Deserialize,
    Serialize,
};

use super::YomineError;
use crate::persistence::{
    atomic_write,
    get_profile_dir,
};

pub const DEFAULT_IGNORED_TERMS: &[&str] = &[
    "の", "は", "に", "へ", "を", "て", "が", "だ", "た", "と", "から", "も", "で", "か", "です",
    "ね", "な", "ん", "し", "お",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IgnoreFile {
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreListData {
    pub ignored_terms: Vec<String>,
    #[serde(default)]
    pub files: Vec<IgnoreFile>,
}

impl Default for IgnoreListData {
    fn default() -> Self {
        Self {
            ignored_terms: DEFAULT_IGNORED_TERMS.iter().map(|s| s.to_string()).collect(),
            files: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct IgnoreList {
    data: IgnoreListData,
    ignored_lookup: HashSet<String>,
    cached_file_terms: HashSet<String>,
}

impl IgnoreList {
    pub fn load() -> Result<Self, YomineError> {
        let file_path = Self::get_ignore_list_path();

        let data = if file_path.exists() {
            let content = fs::read_to_string(&file_path)
                .map_err(|e| YomineError::Custom(format!("Failed to read ignore list: {}", e)))?;

            serde_json::from_str::<IgnoreListData>(&content)
                .map_err(|e| YomineError::Custom(format!("Failed to parse ignore list: {}", e)))?
        } else {
            let default_data = IgnoreListData::default();
            Self::write(&default_data)?;
            default_data
        };

        let mut instance = Self {
            ignored_lookup: data.ignored_terms.iter().cloned().collect(),
            data,
            cached_file_terms: HashSet::new(),
        };
        instance.reload_file_cache();
        Ok(instance)
    }

    pub fn save(&self) -> Result<(), YomineError> {
        Self::write(&self.data)
    }

    fn write(data: &IgnoreListData) -> Result<(), YomineError> {
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| YomineError::Custom(format!("Failed to serialize ignore list: {}", e)))?;

        atomic_write(&Self::get_ignore_list_path(), content.as_bytes())
            .map_err(|e| YomineError::Custom(format!("Failed to write ignore list: {}", e)))
    }

    pub fn add_term(&mut self, term: &str) -> Result<bool, YomineError> {
        let term_string = term.to_string();
        if !self.ignored_lookup.insert(term_string.clone()) {
            return Ok(false);
        }

        self.data.ignored_terms.insert(0, term_string);
        self.save()?;
        Ok(true)
    }

    pub fn remove_term(&mut self, term: &str) -> Result<bool, YomineError> {
        if !self.ignored_lookup.remove(term) {
            return Ok(false);
        }

        self.data.ignored_terms.retain(|x| x != term);
        self.save()?;
        Ok(true)
    }

    pub fn contains(&self, term: &str) -> bool {
        self.ignored_lookup.contains(term) || self.cached_file_terms.contains(term)
    }

    pub fn get_all_terms(&self) -> Vec<String> {
        self.data.ignored_terms.clone()
    }

    pub fn clear_all(&mut self) -> Result<(), YomineError> {
        self.set_terms(Vec::new())
    }

    pub fn set_terms(&mut self, terms: Vec<String>) -> Result<(), YomineError> {
        self.ignored_lookup = terms.iter().cloned().collect();
        self.data.ignored_terms = terms;
        self.save()
    }

    pub fn get_files(&self) -> Vec<IgnoreFile> {
        self.data.files.clone()
    }

    pub fn set_files(&mut self, files: Vec<IgnoreFile>) -> Result<(), YomineError> {
        self.data.files = files;
        self.save()?;
        self.reload_file_cache();
        Ok(())
    }

    pub fn file_exists(path: &str) -> bool {
        PathBuf::from(path).exists()
    }

    pub fn load_terms_from_file(path: &str) -> Result<Vec<String>, YomineError> {
        let content = fs::read_to_string(path)
            .map_err(|e| YomineError::Custom(format!("Failed to read file {}: {}", path, e)))?;

        Ok(content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect())
    }

    pub fn reload_file_cache(&mut self) {
        self.cached_file_terms.clear();

        for file in &self.data.files {
            if file.enabled {
                if let Ok(terms) = Self::load_terms_from_file(&file.path) {
                    self.cached_file_terms.extend(terms);
                }
            }
        }
    }

    fn get_ignore_list_path() -> PathBuf {
        get_profile_dir().join("ignore_list.json")
    }
}
