use std::{
    collections::HashMap,
    sync::{
        mpsc,
        Arc,
        Mutex,
    },
    thread,
};

use tokio::runtime::Runtime;

use super::TaskResult;
use crate::{
    anki::FieldMapping,
    core::{
        IgnoreList, Sentence, SourceFile, Term, pipeline::{
            FilterResult, apply_filters, process_source_file
        }
    },
    gui::LanguageTools,
};

pub struct TaskManager {
    runtime: Arc<Runtime>,
    receiver: mpsc::Receiver<TaskResult>,
    sender: mpsc::Sender<TaskResult>,
}

impl TaskManager {
    pub fn new() -> Self {
        let runtime = Arc::new(Runtime::new().expect("Failed to create TaskManager runtime"));

        let (sender, receiver) = mpsc::channel();

        Self { runtime, receiver, sender }
    }

    pub fn poll_results(&mut self) -> Vec<TaskResult> {
        let mut results = Vec::new();

        while let Ok(result) = self.receiver.try_recv() {
            results.push(result);
        }

        results
    }

    fn task_context(&self) -> (mpsc::Sender<TaskResult>, Arc<Runtime>) {
        (self.sender.clone(), self.runtime.clone())
    }

    pub fn load_language_tools(&self) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            use std::sync::Arc;

            use crate::{
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

    pub fn process_file(
        &self,
        source_file: SourceFile,
        model_mapping: HashMap<String, FieldMapping>,
        language_tools: LanguageTools,
    ) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            let result = runtime.block_on(async {
                process_source_file(&source_file, model_mapping, &language_tools)
                    .await
                    .map_err(|e| e.to_string())
            });

            let _ = sender.send(TaskResult::FileProcessing(result));
        });
    }

    pub fn check_anki_connection(&self) {
        let (sender, runtime) = self.task_context();

        thread::spawn(move || {
            let connected =
                runtime.block_on(async { crate::anki::api::get_version().await.is_ok() });

            let _ = sender.send(TaskResult::AnkiConnection(connected));
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
            let result: Result<(FilterResult, Vec<Sentence>, f32), String> = runtime.block_on(async {
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
                    sentences.iter().map(|s| s.comprehension).sum::<f32>() / sentences.len() as f32
                } else {
                    0.0
                };
                println!("Overall comprehension (refresh): {:.1}%", avg_comprehension * 100.0);

                Ok((filter_result, sentences, avg_comprehension))
            });

            let _ = sender.send(TaskResult::TermsRefreshed(result));
        });
    }

    pub fn request_refresh(&self) {
        let (sender, _) = self.task_context();
        let _ = sender.send(TaskResult::RequestRefresh);
    }

    pub fn request_save_settings(&self) {
        let (sender, _) = self.task_context();
        let _ = sender.send(TaskResult::RequestSaveSettings);
    }
}
