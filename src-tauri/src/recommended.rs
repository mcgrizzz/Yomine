//! Recommended-dictionaries catalog (issue #100). Source of truth is the
//! manifest in the Yomine repo (changeable without an app release); the same
//! file is compiled in as the offline fallback. Entries with an `index_url`
//! get their latest revision checked live.

use serde::{
    Deserialize,
    Serialize,
};

pub const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/mcgrizzz/Yomine/main/assets/recommended_dictionaries.json";

/// The repo copy of the manifest, baked in at compile time.
pub const BAKED_MANIFEST: &str = include_str!("../../assets/recommended_dictionaries.json");

#[derive(Serialize, Deserialize, Clone)]
pub struct RecommendedEntry {
    /// Display name ("JPDB v2.2 (Kana)").
    pub name: String,
    /// The dictionary's `index.json` title — matches it to an installed dict.
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub download_url: String,
    /// Latest known revision. Static manifest data unless `index_url` refreshes it.
    #[serde(default)]
    pub latest_revision: Option<String>,
    /// Live update-index endpoint (Yomitan `indexUrl`); its `revision` field
    /// overrides `latest_revision` when reachable.
    #[serde(default)]
    pub index_url: Option<String>,
}

#[derive(Deserialize)]
struct RecommendedManifest {
    dictionaries: Vec<RecommendedEntry>,
}

pub fn parse_manifest(text: &str) -> Result<Vec<RecommendedEntry>, String> {
    serde_json::from_str::<RecommendedManifest>(text)
        .map(|m| m.dictionaries)
        .map_err(|e| format!("Invalid recommended-dictionaries manifest: {e}"))
}
