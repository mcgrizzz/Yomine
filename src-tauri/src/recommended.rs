//! Recommended-dictionaries manifest (T064, issue #100).
//!
//! The catalog of hosted frequency dictionaries the manager offers for one-click
//! install/update. Source of truth is the manifest in the Yomine repo (so the
//! list/versions can change without an app release), with the same file compiled
//! in as an offline / pre-publish fallback. Entries with an `index_url` (the
//! Yomitan `isUpdatable` convention, e.g. jiten.moe) get their latest revision
//! checked live; otherwise `latest_revision` in the manifest is authoritative.

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
