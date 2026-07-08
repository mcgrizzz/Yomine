//! Update check against the newest GitHub release. Check-only: it never
//! downloads anything, the UI just links to the release page.

use serde::Serialize;
use yomine::core::http::fetch_text;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/mcgrizzz/Yomine/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/mcgrizzz/Yomine/releases";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub url: String,
}

/// `Ok(None)` = up to date. `releases/latest` excludes drafts and prereleases,
/// so beta tags never trigger the notice.
#[tauri::command]
pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let body = tauri::async_runtime::spawn_blocking(|| fetch_text(LATEST_RELEASE_API))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let tag = json["tag_name"].as_str().ok_or("latest release has no tag_name")?;
    let url = json["html_url"].as_str().unwrap_or(RELEASES_PAGE).to_string();

    if is_newer(tag.trim_start_matches('v'), CURRENT_VERSION) {
        Ok(Some(UpdateInfo { current: CURRENT_VERSION.to_string(), latest: tag.to_string(), url }))
    } else {
        Ok(None)
    }
}

/// Numeric x.y.z comparison; trailing non-digits in a segment are ignored and
/// missing segments count as 0.
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (l, c) = (parse(latest), parse(current));
    for i in 0..l.len().max(c.len()) {
        let a = l.get(i).copied().unwrap_or(0);
        let b = c.get(i).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.6.1", "0.6.0"));
        assert!(is_newer("0.7.0", "0.6.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.6.1", "0.6")); // missing segment counts as 0
        assert!(!is_newer("0.6.0", "0.6.0"));
        assert!(!is_newer("0.5.9", "0.6.0"));
        assert!(!is_newer("0.6.0b2", "0.6.0")); // trailing non-digits ignored
    }
}
