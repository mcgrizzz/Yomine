use std::{
    collections::HashMap,
    sync::{
        mpsc,
        Arc,
    },
    thread,
};

use tokio::runtime::Runtime;

use super::TaskResult;
use crate::{
    anki::FieldMapping,
    core::{
        pipeline::process_source_file,
        SourceFile,
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
                let tokenizer = Arc::new(init_vibrato(&dict_type).map_err(|e| e.to_string())?);

                let _ = sender.send(TaskResult::LoadingMessage(
                    "Loading frequency dictionaries...".to_string(),
                ));

                let frequency_manager = Arc::new(
                    frequency_manager::process_frequency_dictionaries()
                        .map_err(|e| e.to_string())?,
                );

                let _ = sender.send(TaskResult::LoadingMessage(
                    "Language tools loaded successfully!".to_string(),
                ));

                Ok::<LanguageTools, String>(LanguageTools { tokenizer, frequency_manager })
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
}
