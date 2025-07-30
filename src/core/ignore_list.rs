use std::{
    fs,
    path::PathBuf,
};

use serde::{
    Deserialize,
    Serialize,
};

use super::YomineError;
use crate::persistence::get_app_data_dir;

pub const DEFAULT_IGNORED_TERMS: &[&str] = &[
    "の", "は", "に", "へ", "を", "て", "が", "だ", "た", "と", "から", "も", "で", "か", "です",
    "ね", "な",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreListData {
    pub ignored_terms: Vec<String>,
}

impl Default for IgnoreListData {
    fn default() -> Self {
        Self { ignored_terms: DEFAULT_IGNORED_TERMS.iter().map(|s| s.to_string()).collect() }
    }
}

#[derive(Debug)]
pub struct IgnoreList {
    data: IgnoreListData,
    file_path: PathBuf,
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
            let instance = Self { data: default_data.clone(), file_path: file_path.clone() };
            instance.save()?;
            default_data
        };

        Ok(Self { data, file_path })
    }

    pub fn save(&self) -> Result<(), YomineError> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                YomineError::Custom(format!("Failed to create ignore list directory: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(&self.data)
            .map_err(|e| YomineError::Custom(format!("Failed to serialize ignore list: {}", e)))?;

        fs::write(&self.file_path, content)
            .map_err(|e| YomineError::Custom(format!("Failed to write ignore list: {}", e)))
    }

    pub fn add_term(&mut self, term: &str) -> Result<bool, YomineError> {
        let term_string = term.to_string();
        if !self.data.ignored_terms.contains(&term_string) {
            self.data.ignored_terms.insert(0, term_string);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn remove_term(&mut self, term: &str) -> Result<bool, YomineError> {
        if let Some(pos) = self.data.ignored_terms.iter().position(|x| x == term) {
            self.data.ignored_terms.remove(pos);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn contains(&self, term: &str) -> bool {
        self.data.ignored_terms.iter().any(|t| t == term)
    }

    pub fn get_all_terms(&self) -> Vec<String> {
        self.data.ignored_terms.clone()
    }

    pub fn clear_all(&mut self) -> Result<(), YomineError> {
        self.data.ignored_terms.clear();
        self.save()
    }

    fn get_ignore_list_path() -> PathBuf {
        get_app_data_dir().join("ignore_list.json")
    }
}
