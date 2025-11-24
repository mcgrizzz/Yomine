use std::path::{
    Path,
    PathBuf,
};

pub const SMOOTHING_ALPHA: f32 = 0.3;
pub const SMOOTHING_BETA: f32 = 0.7;

pub fn find_supported_files_recursive(dir: &Path) -> Vec<PathBuf> {
    use crate::core::models::SourceFileType;

    let mut files = Vec::new();
    let supported_extensions = SourceFileType::supported_extensions();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(find_supported_files_recursive(&path));
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if supported_extensions.contains(&ext) {
                        files.push(path);
                    }
                }
            }
        }
    }

    files
}
pub fn calculate_progress_fraction(current: usize, total: usize) -> f32 {
    if total > 0 {
        current as f32 / total as f32
    } else {
        0.0
    }
}

pub fn calculate_smoothed_time_estimate(
    bytes_processed: u64,
    total_bytes: u64,
    elapsed_secs: f32,
    previous_estimate: Option<f32>,
) -> Option<f32> {
    if bytes_processed == 0 || total_bytes == 0 {
        return None;
    }

    let bytes_per_sec = bytes_processed as f32 / elapsed_secs;
    let remaining_bytes = total_bytes.saturating_sub(bytes_processed);
    let raw_estimate = remaining_bytes as f32 / bytes_per_sec;

    let smoothed = match previous_estimate {
        Some(prev) => SMOOTHING_ALPHA * raw_estimate + SMOOTHING_BETA * prev,
        None => raw_estimate,
    };

    Some(smoothed)
}
