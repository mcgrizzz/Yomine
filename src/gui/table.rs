use eframe::egui::{self, text::LayoutJob, TextFormat, TextStyle};
use egui_extras::{Column, TableBuilder, TableRow};
use wana_kana::ConvertJapanese;

use crate::core::Term;

use super::YomineApp;

pub struct TableState {
    sort: TableSort,
}

impl Default for TableState {
    fn default() -> Self {
        Self {
            sort: TableSort::FrequencyAscending
        }
    }
}

enum TableSort {
    FrequencyDescending,
    FrequencyAscending,
}

impl TableSort {
    fn text(&self) -> String {
        match &self {
            TableSort::FrequencyAscending => "⬆".to_string(),
            TableSort::FrequencyDescending => "⬇".to_string(),
        }
    }

    fn click(&self) -> Self {
        match &self {
            TableSort::FrequencyAscending => TableSort::FrequencyDescending,
            TableSort::FrequencyDescending => TableSort::FrequencyAscending,
        }
    }
}

impl Default for TableSort {
    fn default() -> Self {
        Self::FrequencyAscending
    }
}

fn col_term(ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        ui.label(app.theme.bold(&term.lemma_form).size(22.0))
        .on_hover_ui_at_pointer(|ui| {
            ui.label(app.theme.heading(&term.lemma_reading.to_hiragana()));
            ui.label(app.theme.heading(&term.lemma_reading.to_katakana()));
        });
    });
}

fn col_sentence(ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {

        if let None = term.sentence_references.get(0){
            return;
        }

        let sentence = term.sentence_references.get(0).unwrap();
        let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
        let surface_index = sentence.1;
        
        let preslice = &sentence_content.text[..surface_index];
        let endslice = &sentence_content.text[surface_index + &term.full_segment.len()..];

        let mut job = LayoutJob::default();

        // Define text formats
        let normal_format = TextFormat {
            font_id: ctx.style().text_styles.get(&TextStyle::Body).unwrap().clone(),
            color: ctx.style().visuals.widgets.noninteractive.fg_stroke.color,
            ..Default::default()
        };

        let highlighted_format = TextFormat {
            font_id: ctx.style().text_styles.get(&TextStyle::Body).unwrap().clone(),
            color: app.theme.red(), // Highlighted color
            ..Default::default()
        };

        job.append(preslice, 0.0, normal_format.clone());
        job.append(&term.surface_form, 0.0, highlighted_format);
        job.append(endslice, 0.0, normal_format);
        ui.label(job)
            .on_hover_ui_at_pointer(|ui| {
                ui.label(app.theme.heading(&&term.surface_reading.to_hiragana()));
            });

    });
}

fn format_human_timestamp(timestamp: &str) -> String {
    if let Ok(seconds) = crate::websocket::WebSocketServer::convert_srt_timestamp_to_seconds(timestamp) {
        // Extract hours, minutes, seconds
        let hours = (seconds / 3600.0).floor() as u32;
        let minutes = ((seconds % 3600.0) / 60.0).floor() as u32;
        let secs = (seconds % 60.0).floor() as u32;
        
        // Format based on components
        let formatted = if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, secs)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}s", secs)
        };
        
        format!("{:<11}", formatted)
    } else {
        format!("{:<11}", timestamp)
    }
}

fn col_timestamp(ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        if let None = term.sentence_references.get(0){
            return;
        }
        
        let sentence = term.sentence_references.get(0).unwrap();
        let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
        
        if let Some(timestamp) = &sentence_content.timestamp {
            if app.websocket_state.has_clients && app.websocket_server.is_some() {
                // If we have connected clients, show a clickable button
                // Check if this timestamp has been confirmed
                
                // Extract the first part of the timestamp if it contains an arrow
                let clean_timestamp = timestamp.split(" --> ").next().unwrap_or(timestamp);
                
                // Format the timestamp in a more human-readable way
                let human_timestamp = format_human_timestamp(clean_timestamp);

                let is_confirmed = app.websocket_state.confirmed_timestamps.contains(&clean_timestamp.to_string());
                
                // Color based on confirmation status
                let button_text = if is_confirmed {
                    format!("👁 {}", human_timestamp)  // Eye for confirmed
                } else {
                    format!("▶ {}", human_timestamp)  // Play button for not confirmed
                };
                
                // Use a visually distinct button for confirmed timestamps
                let mut button = egui::Button::new(button_text);
                if is_confirmed {
                    button = button.fill(egui::Color32::from_hex("#71778a").unwrap());
                }
                
                let response = ui.add(button);
                
                // Show original timestamp on hover
                
                if response.clicked() {
                    if let Some(server) = &app.websocket_server {
                        if let Ok(seconds) = crate::websocket::WebSocketServer::convert_srt_timestamp_to_seconds(clean_timestamp) {
                            // Send the timestamp to all connected clients
                            match server.seek_timestamp(seconds, clean_timestamp) {
                                Ok(_) => {
                                    println!("Sent seek command for timestamp: {}", clean_timestamp);
                                },
                                Err(e) => {
                                    eprintln!("Error sending seek command: {:?}", e);
                                }
                            }
                        } else {
                            eprintln!("Failed to convert timestamp: {} to seconds", clean_timestamp);
                        }
                    } else {
                        println!("WebSocket server not available");
                    }
                }
            } else {
                // If no clients connected, just show the timestamp in human-readable format
                let timestamp_vec: Vec<&str> = timestamp.split(" --> ").collect();
                let human_timestamp = format!("{}", format_human_timestamp(timestamp_vec[0]));
                
                // Display the human-readable timestamp with original as hover text
                ui.label(&human_timestamp);
            }
        }
    });
}

fn col_frequency(ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        if let Some(&freq) = term.frequencies.get("HARMONIC") {
            ui.label(if freq == u32::MAX { "？".to_string() } else { freq.to_string() });
        }
    });
}

fn col_pos(ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    let pos = term.part_of_speech.to_string();

    row.col(|ui| {
        ui.label(pos);
    });
}


pub fn term_table(ctx: &egui::Context, app: &mut YomineApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Term Table");
        egui::ScrollArea::vertical().show(ui, |ui| {
            TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(100.0))
            .column(Column::auto().at_least(150.0))
            .column(Column::auto().at_least(40.0))
            .column(Column::auto().at_least(40.0))
            .column(Column::remainder())
            .header(25.0, |mut header| {
                header_cols(ctx, header, app);
            })
            .body(|mut body| {
                let row_height = |i: usize| {
                    let t = &app.terms[i];

                    if let None = t.sentence_references.get(0){
                        return 36.0;
                    }

                    let sentence = t.sentence_references.get(0).unwrap();
                    let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
                    let lines: Vec<&str> = sentence_content.text.trim().split("\n").collect();
                    36.0_f32.max(18.0 * (lines.len() as f32)) //Size 22.0 font is not 22 height.. 
                };

                body.heterogeneous_rows((0..app.terms.iter().len()).map(row_height), |mut row| {
                    let t = &app.terms[row.index()];
                    col_term(ctx, &mut row, t, app);
                    col_sentence(ctx, &mut row, t, app);
                    col_timestamp(ctx, &mut row, t, app);
                    col_frequency(ctx, &mut row, t, app);
                    col_pos(ctx, &mut row, t, app);
                });
            });
        });
        
    });
}

pub fn header_cols(ctx: &egui::Context, mut header: TableRow<'_, '_>, app: &mut YomineApp) {
    header.col(|ui| {
        ui.label(app.theme.heading("Term"));
    });
    header.col(|ui| {
        ui.label(app.theme.heading("Sentence"));
    });
    header.col(|ui| {
        ui.label(app.theme.heading("Timestamp"));
    });
    header.col(|ui| {
        egui::Sides::new().height(25.0).show(
            ui,
            |ui| {
                if ui.button(app.table_state.sort.text()).clicked() {
                    app.table_state.sort = app.table_state.sort.click();
                    app.terms.sort_unstable_by(|a, b| match app.table_state.sort {
                    TableSort::FrequencyAscending => {
                        let freq_a = a.frequencies.get("HARMONIC").unwrap();
                        let freq_b = b.frequencies.get("HARMONIC").unwrap();
                        freq_a.cmp(freq_b) // Ascending order
                    }
                    TableSort::FrequencyDescending => {
                        let freq_a = a.frequencies.get("HARMONIC").unwrap();
                        let freq_b = b.frequencies.get("HARMONIC").unwrap();
                        freq_b.cmp(freq_a) // Descending order
                    }
                });
                }
            },
            |ui| {
                ui.label(app.theme.heading("Frequency"));
            },
        );
        
    });

    header.col(|ui| {
        ui.label(app.theme.heading("Part of Speech"));
    });
}