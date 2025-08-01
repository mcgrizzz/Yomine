use std::sync::mpsc::Receiver;

use eframe::egui;

use super::data::AnkiModelInfo;

pub struct AnkiService {
    pub is_loading_models: bool,
    pub connection_status: String,
    pub model_fetch_receiver: Option<Receiver<Result<Vec<AnkiModelInfo>, String>>>,
    pub sample_fetch_receiver:
        Option<Receiver<(String, Option<std::collections::HashMap<String, String>>)>>,
}

impl AnkiService {
    pub fn new() -> Self {
        Self {
            is_loading_models: false,
            connection_status: "Ready".to_string(),
            model_fetch_receiver: None,
            sample_fetch_receiver: None,
        }
    }

    pub fn fetch_models(&mut self, ctx: &egui::Context) {
        if self.is_loading_models {
            return;
        }

        self.is_loading_models = true;
        self.connection_status = "Fetching models...".to_string();

        let (sender, receiver) = std::sync::mpsc::channel();
        self.model_fetch_receiver = Some(receiver);

        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                match crate::anki::api::get_version().await {
                    Ok(_) => match crate::anki::get_models().await {
                        Ok(models) => {
                            // Convert Model to AnkiModelInfo for UI compatibility
                            let model_info: Vec<AnkiModelInfo> = models
                                .into_iter()
                                .map(|model| AnkiModelInfo {
                                    name: model.name,
                                    fields: model.fields,
                                    sample_note: model.sample_note,
                                })
                                .collect();
                            Ok(model_info)
                        }
                        Err(e) => Err(format!("Failed to fetch models: {}", e)),
                    },
                    Err(_) => Err("Anki Offline".to_string()),
                }
            });

            let _ = sender.send(result);
            ctx_clone.request_repaint();
        });
    }

    pub fn fetch_sample_note(&mut self, model_name: &str, ctx: &egui::Context) {
        if self.is_loading_models {
            return;
        }

        let model_name = model_name.to_string();
        let ctx_clone = ctx.clone();

        let (sender, receiver) = std::sync::mpsc::channel();
        self.sample_fetch_receiver = Some(receiver);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let sample_note = rt.block_on(async {
                crate::anki::get_sample_note_for_model(&model_name).await.unwrap_or(None)
            });

            let _ = sender.send((model_name, sample_note));
            ctx_clone.request_repaint();
        });
    }

    pub fn check_async_results(
        &mut self,
        available_models: &mut Vec<AnkiModelInfo>,
        ctx: &egui::Context,
    ) -> bool {
        let mut updated = false;

        if let Some(ref receiver) = self.model_fetch_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(models) => {
                        *available_models = models;
                        self.connection_status = "Connected".to_string();
                    }
                    Err(error) => {
                        self.connection_status = format!("Error: {}", error);
                    }
                }
                self.is_loading_models = false;
                self.model_fetch_receiver = None;
                updated = true;
            }
        }

        if let Some(ref receiver) = self.sample_fetch_receiver {
            if let Ok((model_name, sample_note)) = receiver.try_recv() {
                if let Some(model) = available_models.iter_mut().find(|m| m.name == model_name) {
                    model.sample_note = sample_note;
                }
                self.sample_fetch_receiver = None;
                updated = true;
            }
        }

        if updated {
            ctx.request_repaint();
        }

        updated
    }
}

impl Default for AnkiService {
    fn default() -> Self {
        Self::new()
    }
}
