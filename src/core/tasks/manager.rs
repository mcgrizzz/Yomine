use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::AtomicBool,
        mpsc,
        Arc,
        Mutex,
    },
    thread,
};

use tokio::runtime::Runtime;

use super::{
    types::{
        CancellableTask,
        TaskHandle,
    },
    TaskResult,
};
use crate::{
    anki::FieldMapping,
    core::{
        pipeline::FilterResult,
        Sentence,
        SourceFile,
        Term,
    },
    gui::LanguageTools,
};

pub struct TaskManager {
    runtime: Arc<Runtime>,
    receiver: mpsc::Receiver<TaskResult>,
    sender: mpsc::Sender<TaskResult>,
    active_tasks: Arc<Mutex<HashMap<CancellableTask, TaskHandle>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        let runtime = Arc::new(Runtime::new().expect("Failed to create TaskManager runtime"));

        let (sender, receiver) = mpsc::channel();

        Self { runtime, receiver, sender, active_tasks: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn poll_results(&mut self) -> Vec<TaskResult> {
        let mut results = Vec::new();

        while let Ok(result) = self.receiver.try_recv() {
            results.push(result);
        }

        let mut tasks = self.active_tasks.lock().unwrap();
        tasks.retain(|_, handle| !handle.is_finished());

        results
    }

    fn task_context(&self) -> (mpsc::Sender<TaskResult>, Arc<Runtime>) {
        (self.sender.clone(), self.runtime.clone())
    }

    /// Spawn a long-running task and store its handle in the active tasks map
    /// Returns the cancel token that can be passed to the task
    fn spawn_task<F>(&self, cancellable_task: CancellableTask, task_fn: F) -> Arc<AtomicBool>
    where
        F: FnOnce(Arc<AtomicBool>) + Send + 'static,
    {
        let cancel_token = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel_token);

        let join_handle = thread::spawn(move || {
            task_fn(cancel_clone);
        });

        let handle = TaskHandle::new(Arc::clone(&cancel_token), join_handle);

        let mut tasks = self.active_tasks.lock().unwrap();
        tasks.insert(cancellable_task, handle);

        cancel_token
    }

    pub fn cancel_task(&self, cancellable_task: CancellableTask) {
        let tasks = self.active_tasks.lock().unwrap();
        if let Some(handle) = tasks.get(&cancellable_task) {
            handle.cancel();
        }
    }

    //Task specific methods

    pub fn request_refresh(&self) {
        let (sender, _) = self.task_context();
        let _ = sender.send(TaskResult::RequestRefresh);
    }

    pub fn request_save_settings(&self) {
        let (sender, _) = self.task_context();
        let _ = sender.send(TaskResult::RequestSaveSettings);
    }

    pub fn check_anki_connection(&self) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            let connected =
                runtime.block_on(async { crate::anki::api::get_version().await.is_ok() });

            let _ = sender.send(TaskResult::AnkiConnection(connected));
        });
    }

    pub fn process_file(
        &self,
        source_file: SourceFile,
        model_mapping: HashMap<String, FieldMapping>,
        language_tools: LanguageTools,
    ) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            use crate::core::pipeline::process_source_file;

            let result = runtime.block_on(async {
                process_source_file(&source_file, model_mapping, &language_tools)
                    .await
                    .map_err(|e| e.to_string())
            });

            let _ = sender.send(TaskResult::FileProcessing(result));
        });
    }

    pub fn load_language_tools(&self) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            use std::sync::Arc;

            use crate::{
                core::IgnoreList,
                dictionary::{
                    frequency_manager,
                    token_dictionary::DictType,
                },
                gui::app::LanguageTools,
                segmentation::tokenizer::init_vibrato,
            };

            let _ = sender.send(TaskResult::LoadingMessage("Loading tokenizer...".to_string()));

            let result = runtime.block_on(async {
                let dict_type = DictType::Unidic;

                let sender_clone = sender.clone();
                let progress_callback = Box::new(move |message: String| {
                    let _ = sender_clone.send(TaskResult::LoadingMessage(message));
                });

                let tokenizer = Arc::new(
                    init_vibrato(&dict_type, Some(progress_callback)).map_err(|e| e.to_string())?,
                );

                let _ = sender.send(TaskResult::LoadingMessage(
                    "Loading frequency dictionaries...".to_string(),
                ));

                let frequency_manager = Arc::new(
                    frequency_manager::process_frequency_dictionaries()
                        .map_err(|e| e.to_string())?,
                );

                let _ =
                    sender.send(TaskResult::LoadingMessage("Loading ignore list...".to_string()));

                let ignore_list =
                    Arc::new(Mutex::new(IgnoreList::load().map_err(|e| e.to_string())?));

                let _ = sender.send(TaskResult::LoadingMessage(
                    "Language tools loaded successfully!".to_string(),
                ));

                Ok::<LanguageTools, String>(LanguageTools {
                    tokenizer,
                    frequency_manager,
                    ignore_list,
                    known_interval: 0, // Will be set from settings after loading
                })
            });

            let _ = sender.send(TaskResult::LanguageToolsLoaded(result));
        });
    }

    pub fn refresh_terms(
        &self,
        base_terms: Vec<Term>,
        mut sentences: Vec<Sentence>,
        model_mapping: HashMap<String, FieldMapping>,
        language_tools: LanguageTools,
    ) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            use crate::core::pipeline::apply_filters;
            let result: Result<(FilterResult, Vec<Sentence>, f32), String> =
                runtime.block_on(async {
                    let filter_result =
                        apply_filters(base_terms, &language_tools, Some(model_mapping), None)
                            .await
                            .map_err(|e| e.to_string())?;

                    // Reconstruct all terms from filter result
                    let mut all_terms = Vec::new();
                    all_terms.extend(filter_result.terms.iter().cloned());
                    all_terms.extend(filter_result.anki_filtered.iter().cloned());
                    all_terms.extend(filter_result.ignore_filtered.iter().cloned());

                    // Recalculate sentence comprehension
                    {
                        use crate::anki::comprehensibility::calculate_sentence_comprehension;
                        for sentence in &mut sentences {
                            calculate_sentence_comprehension(sentence, &all_terms);
                        }
                    }

                    // Calculate and print overall comprehension
                    let avg_comprehension = if !sentences.is_empty() {
                        sentences.iter().map(|s| s.comprehension).sum::<f32>()
                            / sentences.len() as f32
                    } else {
                        0.0
                    };
                    println!("Overall comprehension (refresh): {:.1}%", avg_comprehension * 100.0);

                    Ok((filter_result, sentences, avg_comprehension))
                });

            let _ = sender.send(TaskResult::TermsRefreshed(result));
        });
    }

    pub fn analyze_frequency(&self, file_paths: Vec<PathBuf>, language_tools: LanguageTools) {
        let (sender, runtime) = self.task_context();

        self.spawn_task(CancellableTask::FrequencyAnalysis, move |cancel_token| {
            use crate::{
                core::tasks::types::{
                    AnalysisProgress,
                    FrequencyAnalysisUpdate,
                },
                tools::analysis::analyzer::analyze_files,
            };

            let result = runtime.block_on(async {
                let sender_clone = sender.clone();
                let progress_callback =
                    Box::new(move |current_file: usize, message: String, file_size: u64| {
                        let _ = sender_clone.send(TaskResult::FrequencyAnalysis(
                            FrequencyAnalysisUpdate::Progress(AnalysisProgress {
                                current_file,
                                message,
                                file_size,
                            }),
                        ));
                    });

                analyze_files(
                    file_paths,
                    &language_tools,
                    Some(progress_callback),
                    Some(cancel_token),
                )
                .map_err(|e| e.to_string())
            });

            let _ = sender
                .send(TaskResult::FrequencyAnalysis(FrequencyAnalysisUpdate::Complete(result)));
        });
    }

    pub fn export_frequency(
        &self,
        result: crate::tools::analysis::FrequencyAnalysisResult,
        output_dir: PathBuf,
        dict_name: String,
        dict_author: String,
        dict_url: String,
        dict_description: String,
        revision_prefix: String,
        export_yomitan: bool,
        export_csv_flag: bool,
        pretty_json: bool,
        exclude_hapax: bool,
    ) {
        let (sender, _runtime) = self.task_context();

        thread::spawn(move || {
            use crate::tools::analysis::analyzer::{
                export_csv,
                export_yomitan_zip,
            };

            let mut errors = Vec::new();

            if export_yomitan {
                let author = if dict_author.is_empty() { None } else { Some(dict_author.as_str()) };
                let url = if dict_url.is_empty() { None } else { Some(dict_url.as_str()) };
                let description = if dict_description.is_empty() {
                    None
                } else {
                    Some(dict_description.as_str())
                };
                let revision_prefix_opt =
                    if revision_prefix.is_empty() { None } else { Some(revision_prefix.as_str()) };

                if let Err(e) = export_yomitan_zip(
                    &result,
                    &dict_name,
                    author,
                    url,
                    description,
                    &output_dir,
                    pretty_json,
                    exclude_hapax,
                    revision_prefix_opt,
                ) {
                    errors.push(format!("Yomitan export failed: {}", e));
                }
            }

            if export_csv_flag {
                if let Err(e) = export_csv(&result, &output_dir, &dict_name, exclude_hapax) {
                    errors.push(format!("CSV export failed: {}", e));
                }
            }

            let export_result = if errors.is_empty() {
                Ok(format!("✓ Export successful to: {}", output_dir.display()))
            } else {
                Err(errors.join("\n"))
            };

            let _ = sender.send(TaskResult::FrequencyExport(export_result));
        });
    }
}
