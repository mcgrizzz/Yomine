//! The data root is itself a profile, and additional ones live under `profiles/`,
//! so nothing an existing install already has on disk ever moves.

use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
    sync::RwLock,
};

use serde::{
    Deserialize,
    Serialize,
};

use super::get_app_data_dir;
use crate::core::YomineError;

pub const PROFILES_DIR: &str = "profiles";
const POINTER_FILE: &str = "profiles.json";
const META_FILE: &str = "profile.json";
const DEFAULT_SLUG: &str = "default";
const DEFAULT_DISPLAY: &str = "Default";
const SLUG_PREFIX: &str = "profile-";

/// `dictionaries/` and `asbplayer_subtitles/` are deliberately absent: shared.
const PROFILE_FILES: &[&str] = &[
    "settings.json",
    "ignore_list.json",
    "recent_files.json",
    "epub_history.json",
    "anki_vocab_cache.json",
    "anki_mined_sentences.json",
    "yomine_mined_notes.json",
    "knowledge_summary_cache.json",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Active {
    Root,
    Named(String),
}

#[derive(Serialize, Deserialize, Default)]
struct Pointer {
    active: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Meta {
    display_name: String,
}

pub struct ProfileInfo {
    pub slug: String,
    pub display_name: String,
    pub active: bool,
}

static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

pub fn active() -> Active {
    if let Some(a) = ACTIVE.read().unwrap().clone() {
        return a;
    }
    let mut guard = ACTIVE.write().unwrap();
    if let Some(a) = guard.clone() {
        return a;
    }
    let resolved = resolve(&get_app_data_dir());
    println!("Active profile: {resolved:?}");
    *guard = Some(resolved.clone());
    resolved
}

/// Never for a switch: that profile still exists, and its teardown writes belong to it.
fn repin(a: Active) {
    *ACTIVE.write().unwrap() = Some(a);
}

fn resolve(root: &Path) -> Active {
    match read_pointer(root) {
        Some(slug) if profiles_dir(root).join(&slug).is_dir() => Active::Named(slug),
        _ => Active::Root,
    }
}

fn profiles_dir(root: &Path) -> PathBuf {
    root.join(PROFILES_DIR)
}

fn list_slugs(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(profiles_dir(root)) else {
        return Vec::new();
    };
    let mut slugs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|s| valid_slug(s))
        .collect();
    slugs.sort();
    slugs
}

/// Slugs are generated; `get_profile_dir` would create whatever a hand-edited pointer names.
fn valid_slug(slug: &str) -> bool {
    slug.strip_prefix(SLUG_PREFIX).is_some_and(|n| n.parse::<u32>().is_ok())
}

fn write(value: &impl Serialize, path: &Path, what: &str) -> Result<(), YomineError> {
    super::save_json_at(value, path)
        .map_err(|e| YomineError::Custom(format!("Failed to write the profile {what}: {e}")))
}

fn read_pointer(root: &Path) -> Option<String> {
    let pointer: Pointer = super::load_json_at_or_default(&root.join(POINTER_FILE));
    valid_slug(&pointer.active).then_some(pointer.active)
}

/// An empty slug means the root, which has no directory under `profiles/`.
fn write_pointer(root: &Path, slug: &str) -> Result<(), YomineError> {
    write(&Pointer { active: slug.to_string() }, &root.join(POINTER_FILE), "pointer")
}

fn display_name(dir: &Path) -> Option<String> {
    let meta: Meta = super::load_json_at_or_default(&dir.join(META_FILE));
    Some(meta.display_name).filter(|n| !n.trim().is_empty())
}

fn name_of(root: &Path, slug: &str) -> String {
    let fallback = if slug == DEFAULT_SLUG { DEFAULT_DISPLAY } else { slug };
    display_name(&dir_for(root, slug)).unwrap_or_else(|| fallback.to_string())
}

fn write_display_name(dir: &Path, display_name: &str) -> Result<(), YomineError> {
    write(&Meta { display_name: display_name.to_string() }, &dir.join(META_FILE), "name")
}

fn next_slug(taken: &[String]) -> String {
    (2..)
        .map(|n| format!("{SLUG_PREFIX}{n}"))
        .find(|s| !taken.contains(s))
        .expect("suffixes are unbounded")
}

fn dir_for(root: &Path, slug: &str) -> PathBuf {
    if slug == DEFAULT_SLUG {
        root.to_path_buf()
    } else {
        profiles_dir(root).join(slug)
    }
}

fn active_for(slug: &str) -> Active {
    if slug == DEFAULT_SLUG {
        Active::Root
    } else {
        Active::Named(slug.to_string())
    }
}

fn dir_of(root: &Path, slug: &str) -> Result<PathBuf, YomineError> {
    let dir = dir_for(root, slug);
    (slug == DEFAULT_SLUG || (valid_slug(slug) && dir.is_dir()))
        .then_some(dir)
        .ok_or_else(|| YomineError::Custom(format!("No profile named '{slug}'")))
}

pub fn list() -> Vec<ProfileInfo> {
    let root = get_app_data_dir();
    let current = active();
    std::iter::once(DEFAULT_SLUG.to_string())
        .chain(list_slugs(&root))
        .map(|slug| ProfileInfo {
            display_name: name_of(&root, &slug),
            active: current == active_for(&slug),
            slug,
        })
        .collect()
}

pub fn active_title_name() -> Option<String> {
    let root = get_app_data_dir();
    match active() {
        Active::Root => display_name(&root).map(|name| format!("{name} ({DEFAULT_DISPLAY})")),
        Active::Named(slug) => Some(name_of(&root, &slug)),
    }
}

fn checked_name(display_name: &str) -> Result<&str, YomineError> {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        return Err(YomineError::Custom("Profile name cannot be empty".into()));
    }
    Ok(trimmed)
}

pub fn create(display_name: &str) -> Result<String, YomineError> {
    new_profile(display_name, None)
}

pub fn copy(slug: &str, display_name: &str) -> Result<String, YomineError> {
    new_profile(display_name, Some(slug))
}

fn new_profile(display_name: &str, from: Option<&str>) -> Result<String, YomineError> {
    let name = checked_name(display_name)?;
    let root = get_app_data_dir();
    let src = from.map(|slug| dir_of(&root, slug)).transpose()?;

    let slug = next_slug(&list_slugs(&root));
    let dst = profiles_dir(&root).join(&slug);
    fs::create_dir_all(&dst)
        .map_err(|e| YomineError::Custom(format!("Failed to create profile directory: {e}")))?;

    if let Some(src) = src {
        for file in PROFILE_FILES {
            let from = src.join(file);
            if from.exists() {
                fs::copy(&from, dst.join(file))
                    .map_err(|e| YomineError::Custom(format!("Failed to copy {file}: {e}")))?;
            }
        }
    }
    write_display_name(&dst, name)?;
    write_pointer(&root, &slug)?;
    Ok(slug)
}

pub fn switch(slug: &str) -> Result<(), YomineError> {
    let root = get_app_data_dir();
    dir_of(&root, slug)?;
    write_pointer(&root, if slug == DEFAULT_SLUG { "" } else { slug })
}

pub fn rename(slug: &str, display_name: &str) -> Result<(), YomineError> {
    let name = checked_name(display_name)?;
    let root = get_app_data_dir();
    let dir = dir_of(&root, slug)?;
    write_display_name(&dir, name)
}

/// Returns whether it was the active profile, in which case the caller must relaunch.
pub fn delete(slug: &str) -> Result<bool, YomineError> {
    if slug == DEFAULT_SLUG {
        return Err(YomineError::Custom("The first profile cannot be deleted".into()));
    }
    let root = get_app_data_dir();
    let dir = dir_of(&root, slug)?;

    let was_active = active() == active_for(slug);
    if was_active {
        switch(DEFAULT_SLUG)?;
        // Otherwise `get_profile_dir` recreates the deleted directory before the relaunch.
        repin(Active::Root);
    }
    fs::remove_dir_all(&dir)
        .map_err(|e| YomineError::Custom(format!("Failed to remove profile: {e}")))?;
    Ok(was_active)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{
        AtomicU32,
        Ordering,
    };

    use super::*;

    fn scratch(label: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "yomine-profiles-{}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
            label
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed(root: &Path) {
        for name in PROFILE_FILES {
            fs::write(root.join(name), b"{}").unwrap();
        }
        fs::create_dir_all(root.join("dictionaries")).unwrap();
    }

    #[test]
    fn slugs_are_generated_never_derived_from_the_name() {
        assert_eq!(next_slug(&[]), "profile-2");
        assert_eq!(next_slug(&["profile-2".to_string(), "profile-3".to_string()]), "profile-4");
    }

    #[test]
    fn only_a_generated_slug_is_accepted() {
        assert!(!valid_slug(".."));
        assert!(!valid_slug("../../etc"));
        assert!(!valid_slug("profile-../.."));
        assert!(!valid_slug("a/b"));
        assert!(!valid_slug(""));
        assert!(!valid_slug(DEFAULT_SLUG));
        assert!(valid_slug(&next_slug(&[])));
    }

    #[test]
    fn an_install_with_no_pointer_is_the_root() {
        let root = scratch("no-pointer");
        seed(&root);
        assert_eq!(resolve(&root), Active::Root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_pointer_naming_a_deleted_profile_falls_back_to_the_root() {
        let root = scratch("stale-pointer");
        fs::create_dir_all(profiles_dir(&root).join("profile-2")).unwrap();
        write_pointer(&root, "profile-3").unwrap();

        assert_eq!(resolve(&root), Active::Root);
        assert!(!profiles_dir(&root).join("profile-3").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_escaping_pointer_is_ignored() {
        let root = scratch("escape");
        fs::write(root.join(POINTER_FILE), br#"{"active":"../.."}"#).unwrap();
        assert_eq!(resolve(&root), Active::Root);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn only_a_never_named_root_is_absent_from_the_title() {
        let root = scratch("title");
        assert_eq!(display_name(&root), None);

        write_display_name(&root, "Novels").unwrap();
        assert_eq!(display_name(&root).as_deref(), Some("Novels"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_new_profile_never_touches_the_root() {
        let root = scratch("create");
        seed(&root);

        let dst = profiles_dir(&root).join("profile-2");
        fs::create_dir_all(&dst).unwrap();
        write_display_name(&dst, "アニメ / CON.").unwrap();
        write_pointer(&root, "profile-2").unwrap();

        for name in PROFILE_FILES {
            assert!(root.join(name).exists(), "{name} left the root");
            assert!(!dst.join(name).exists(), "{name} reached an empty profile");
        }
        assert!(root.join("dictionaries").is_dir());
        assert_eq!(resolve(&root), Active::Named("profile-2".to_string()));
        assert_eq!(display_name(&dst).as_deref(), Some("アニメ / CON."));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_copy_duplicates_state_without_touching_the_source() {
        let root = scratch("copy");
        fs::write(root.join("settings.json"), br#"{"anki_interval":42}"#).unwrap();

        let dst = profiles_dir(&root).join("profile-2");
        fs::create_dir_all(&dst).unwrap();
        for file in PROFILE_FILES {
            let from = root.join(file);
            if from.exists() {
                fs::copy(&from, dst.join(file)).unwrap();
            }
        }

        let copied = fs::read_to_string(dst.join("settings.json")).unwrap();
        assert_eq!(copied, r#"{"anki_interval":42}"#);
        assert!(root.join("settings.json").exists());
        fs::remove_dir_all(&root).unwrap();
    }
}
