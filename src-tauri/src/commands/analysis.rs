//! Frequency-analyzer commands (contracts/commands.md "Frequency analysis").
//! The engine calls are CPU-heavy and blocking, so they run on `spawn_blocking`
//! with the state lock held only to clone handles and store results.

use std::{
    path::PathBuf,
    sync::{
        atomic::Ordering,
        Arc,
        Mutex,
    },
    time::Instant,
};

use tauri::{
    ipc::Channel,
    AppHandle,
    Emitter,
    State,
};
use yomine::tools::analysis::{
    analyzer::{
        analyze_files,
        export_csv,
        export_yomitan_zip,
    },
    calculate_smoothed_time_estimate,
    find_supported_files_recursive,
    CorpusBalancer,
    ExportOptions,
    FrequencyAnalysisResult,
};

use crate::{
    dto::{
        AnalysisPreview,
        AnalysisPreviewEntry,
        AnalysisProgressDto,
    },
    events::{
        names,
        ExportComplete,
    },
    state::AppState,
};

/// How many preview rows cross the IPC boundary. The full result for export stays in `AppState`.
const PREVIEW_LIMIT: usize = 250;

/// Expand a picked folder to the supported files under it (recursive); the
/// frontend builds its selection tree from this list.
#[tauri::command]
pub fn find_analysis_files(dir: String) -> Vec<String> {
    find_supported_files_recursive(std::path::Path::new(&dir))
        .into_iter()
        .map(|p| p.display().to_string())
        .collect()
}

/// Build the lightweight preview (top `PREVIEW_LIMIT` lemmas by frequency) the UI
/// renders. `total` is the full unique-lemma count before the cap.
fn build_preview(result: &FrequencyAnalysisResult) -> AnalysisPreview {
    let mut entries: Vec<AnalysisPreviewEntry> = result
        .frequencies
        .iter()
        .map(|((term, reading), &count)| AnalysisPreviewEntry {
            term: term.clone(),
            reading: reading.clone(),
            frequency: count,
            count,
        })
        .collect();
    entries.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    let total = entries.len();
    // Bottom slice: the last ≤PREVIEW_LIMIT entries of the same freq-desc list
    // (lowest-frequency terms, still desc order).]` for the Bottom 250 radio.
    let bottom = entries[entries.len().saturating_sub(PREVIEW_LIMIT)..].to_vec();
    entries.truncate(PREVIEW_LIMIT);
    AnalysisPreview { entries, bottom, total }
}

/// Tokenize the corpus and count lemma frequencies. The full result stays in
/// `AppState.last_analysis` for export; only the preview crosses the boundary.
/// User cancel returns `Err` (and emits `analysis-cancelled`).
#[tauri::command]
pub async fn start_analysis(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    paths: Vec<String>,
    balance_corpus: bool,
    progress: Channel<AnalysisProgressDto>,
) -> Result<AnalysisPreview, String> {
    // Brief lock: clone the cheap Arc-backed tools, grab + reset the cancel flag.
    let (tools, cancel_flag) = {
        let guard = state.lock().unwrap();
        let tools = guard
            .language_tools
            .clone()
            .ok_or_else(|| "Language tools are still loading".to_string())?;
        guard.analysis_cancel.store(false, Ordering::Relaxed);
        (tools, Arc::clone(&guard.analysis_cancel))
    };

    let mut file_paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    // Balance first, then pre-sum `total_bytes` so progress/ETA reflect the
    // balanced set.
    if balance_corpus {
        file_paths = CorpusBalancer::new(file_paths).balance();
    }
    let total_files = file_paths.len();

    // Sum file sizes up front so the per-file ETA is computable (egui pre-sums
    // `total_bytes` before starting; the callback only reports per-file sizes).
    let total_bytes: u64 =
        file_paths.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();

    // The blocking engine call calls `progress_callback(file_index_1based,
    // message, file_size)` once per file. We accumulate `bytes_processed`, derive
    // the smoothed ETA from elapsed time, and push an `AnalysisProgressDto`.
    let progress_channel = progress.clone();
    let start_time = Instant::now();
    let bytes_processed = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let prev_estimate = Arc::new(Mutex::new(None::<f32>));

    let callback = {
        let bytes_processed = Arc::clone(&bytes_processed);
        let prev_estimate = Arc::clone(&prev_estimate);
        Box::new(move |current_file: usize, message: String, file_size: u64| {
            let done = bytes_processed.fetch_add(file_size, Ordering::Relaxed) + file_size;
            let elapsed = start_time.elapsed().as_secs_f32();
            let mut prev = prev_estimate.lock().unwrap();
            let eta = calculate_smoothed_time_estimate(done, total_bytes, elapsed, *prev);
            if eta.is_some() {
                *prev = eta;
            }
            let _ = progress_channel.send(AnalysisProgressDto {
                total_files,
                current_file,
                message,
                total_bytes,
                bytes_processed: done,
                eta_secs: eta,
            });
        }) as Box<dyn Fn(usize, String, u64) + Send + Sync>
    };

    // CPU-heavy + synchronous → run off the async runtime on a blocking thread.
    let result = tauri::async_runtime::spawn_blocking(move || {
        analyze_files(file_paths, &tools, Some(callback), Some(cancel_flag))
    })
    .await
    .map_err(|e| format!("Analysis task panicked: {e}"))?;

    match result {
        Ok(result) => {
            let preview = build_preview(&result);
            // Brief lock to store the full result for `export_analysis`.
            state.lock().unwrap().last_analysis = Some(result);
            // Contract lists both the return value and the event — emit for store
            // consistency (other listeners), then return for the caller.
            let _ = app.emit(names::ANALYSIS_COMPLETE, &preview);
            Ok(preview)
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("cancelled") {
                let _ = app.emit(names::ANALYSIS_CANCELLED, ());
            }
            Err(msg)
        }
    }
}

/// Request cancellation of a running `start_analysis`. Flips the shared
/// flag; `analyze_files` checks it per file and returns the cancel `Err`, which
/// emits `analysis-cancelled`.
#[tauri::command]
pub fn cancel_analysis(state: State<'_, Mutex<AppState>>) {
    state.lock().unwrap().analysis_cancel.store(true, Ordering::Relaxed);
}

/// Export the last analysis as a Yomitan zip and/or CSV. Empty option strings
/// map to `None`.
#[tauri::command]
pub async fn export_analysis(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    output_dir: String,
    options: ExportOptions,
) -> Result<String, String> {
    // Brief lock: clone the result out so the blocking export holds no lock.
    let result = state
        .lock()
        .unwrap()
        .last_analysis
        .clone()
        .ok_or_else(|| "No analysis to export".to_string())?;

    let dir = PathBuf::from(&output_dir);

    let export = tauri::async_runtime::spawn_blocking(move || {
        let mut errors: Vec<String> = Vec::new();

        if options.export_yomitan {
            // Empty strings → None for the optional metadata fields.
            let opt = |s: &str| if s.is_empty() { None } else { Some(s.to_string()) };
            let author = opt(&options.dict_author);
            let url = opt(&options.dict_url);
            let description = opt(&options.dict_description);
            let revision_prefix = opt(&options.revision_prefix);
            if let Err(e) = export_yomitan_zip(
                &result,
                &options.dict_name,
                author.as_deref(),
                url.as_deref(),
                description.as_deref(),
                &dir,
                options.pretty_json,
                options.exclude_hapax,
                revision_prefix.as_deref(),
            ) {
                errors.push(format!("Yomitan export failed: {e}"));
            }
        }

        if options.export_csv {
            if let Err(e) = export_csv(&result, &dir, &options.dict_name, options.exclude_hapax) {
                errors.push(format!("CSV export failed: {e}"));
            }
        }

        if errors.is_empty() {
            Ok(format!("✓ Export successful to: {}", dir.display()))
        } else {
            Err(errors.join("\n"))
        }
    })
    .await
    .map_err(|e| format!("Export task panicked: {e}"))?;

    let (ok, message) = match &export {
        Ok(msg) => (true, msg.clone()),
        Err(msg) => (false, msg.clone()),
    };
    let _ = app.emit(names::EXPORT_COMPLETE, ExportComplete { ok, message });
    export
}
