use eframe::egui::{
    self,
    pos2,
    Color32,
    Context,
    RichText,
    Ui,
};
use egui_extras::{
    Column,
    TableBuilder,
    TableRow,
};
use wana_kana::ConvertJapanese;

use super::{
    theme::blend_colors,
    YomineApp,
};
use crate::{
    core::Term,
    segmentation::word::POS,
};

pub struct TableState {
    sort: TableSort,
}

impl Default for TableState {
    fn default() -> Self {
        Self { sort: TableSort::FrequencyAscending }
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
    let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;

    row.col(|ui| {
        ui.label(
            RichText::new(&term.lemma_form)
                .color(blend_colors(normal_color, app.theme.red(), 0.8))
                .size(22.0),
        )
        .on_hover_ui_at_pointer(|ui| {
            ui.label(app.theme.heading(&term.lemma_reading.to_hiragana()));
            ui.label(app.theme.heading(&term.lemma_reading.to_katakana()));
        });
    });
}

fn segment_ui(
    ui: &mut Ui,
    text: &str,
    text_color: Color32,
    underline_color: Option<Color32>,
    highlight: bool,
    hover_text: Option<RichText>,
) {
    let label = egui::Label::new(RichText::new(text).color(text_color));
    let response = ui.add(label);

    if let Some(underline_color) = underline_color {
        let rect = response.rect;
        let thickness = match highlight {
            true => 1.5,
            false => 1.0,
        };

        // Calculate the y-position and x-range for the underline
        let y = rect.left_bottom().y + (thickness / 2.0 + 0.75);
        let x_start = rect.left_bottom().x + 1.5;
        let x_end = rect.right_bottom().x - 1.5;

        if highlight {
            // Draw a solid underline for highlighted segments
            ui.painter()
                .line_segment([pos2(x_start, y), pos2(x_end, y)], (thickness, underline_color));
        } else {
            // Draw a dashed underline for non-highlighted segments
            let dash_length = 2.0; // Length of each dash in points
            let gap_length = 2.0; // Length of each gap in points
            let mut current_x = x_start;

            while current_x < x_end {
                let dash_start = current_x;
                let mut dash_end = current_x + dash_length;
                if dash_end > x_end {
                    dash_end = x_end; // Truncate the last dash if it exceeds the end
                }
                ui.painter().line_segment(
                    [pos2(dash_start, y), pos2(dash_end, y)],
                    (thickness, underline_color),
                );
                current_x = dash_end + gap_length;
            }
        }
    }

    if let Some(hover_text) = hover_text {
        response.on_hover_text(hover_text);
    }
}

fn col_sentence(ctx: &Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        if term.sentence_references.get(0).is_none() {
            return;
        }

        let sentence = term.sentence_references.get(0).unwrap();
        let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
        let surface_index = sentence.1;

        let highlighted_color = app.theme.red();
        let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;
        // Use horizontal layout with no spacing between items
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // Iterate over segments
            for (reading, pos, start, stop) in sentence_content.segments.iter() {
                let segment_text = &sentence_content.text[*start..*stop];
                let is_term = *start == surface_index;

                // Determine colors
                let color = if is_term {
                    blend_colors(normal_color, highlighted_color, 0.95)
                } else {
                    match pos {
                        POS::Verb | POS::SuruVerb => {
                            blend_colors(normal_color, app.theme.blue(), 0.75)
                        }
                        POS::Noun => blend_colors(normal_color, app.theme.green(), 0.75),
                        POS::Adjective | POS::AdjectivalNoun => {
                            blend_colors(normal_color, app.theme.orange(), 0.75)
                        }
                        POS::Adverb => blend_colors(normal_color, app.theme.purple(), 0.75),
                        POS::Postposition => blend_colors(normal_color, Color32::BLACK, 0.25),
                        _ => normal_color,
                    }
                };

                let text_color = if is_term {
                    blend_colors(normal_color, highlighted_color, 0.85)
                } else {
                    blend_colors(normal_color, color, 0.85)
                };

                let hover_text = match reading.as_str() {
                    "*" => None,
                    _ => Some(RichText::new(&format!("{}", reading.to_hiragana())).color(color)),
                };

                match pos {
                    POS::Symbol => {
                        segment_ui(ui, segment_text, text_color, None, is_term, hover_text);
                    }
                    _ => {
                        segment_ui(ui, segment_text, text_color, Some(color), is_term, hover_text);
                    }
                }
            }
        });
    });
}

fn format_human_timestamp(timestamp: &str) -> String {
    if let Ok(seconds) =
        crate::websocket::WebSocketServer::convert_srt_timestamp_to_seconds(timestamp)
    {
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

fn col_timestamp(_ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        if let None = term.sentence_references.get(0) {
            return;
        }

        let sentence = term.sentence_references.get(0).unwrap();
        let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
        if let Some(timestamp) = &sentence_content.timestamp {
            if app.websocket_manager.has_clients() && app.websocket_manager.server.is_some() {
                // If we have connected clients, show a clickable button
                // Check if this timestamp has been confirmed

                // Extract the first part of the timestamp if it contains an arrow
                let clean_timestamp = timestamp.split(" --> ").next().unwrap_or(timestamp);

                // Format the timestamp in a more human-readable way
                let human_timestamp = format_human_timestamp(clean_timestamp);

                let is_confirmed = app
                    .websocket_manager
                    .get_confirmed_timestamps()
                    .contains(&clean_timestamp.to_string());

                // Color based on confirmation status
                let button_text = if is_confirmed {
                    format!("👁 {}", human_timestamp) // Eye for confirmed
                } else {
                    format!("▶ {}", human_timestamp) // Play button for not confirmed
                };

                // Use a visually distinct button for confirmed timestamps
                let mut button = egui::Button::new(button_text);
                if is_confirmed {
                    button = button.fill(egui::Color32::from_hex("#71778a").unwrap());
                }

                let response = ui.add(button);

                // Show original timestamp on hover

                if response.clicked() {
                    if let Some(server) = &app.websocket_manager.server {
                        if let Ok(seconds) =
                            crate::websocket::WebSocketServer::convert_srt_timestamp_to_seconds(
                                clean_timestamp,
                            )
                        {
                            // Send the timestamp to all connected clients
                            match server.seek_timestamp(seconds, clean_timestamp) {
                                Ok(_) => {
                                    println!(
                                        "Sent seek command for timestamp: {}",
                                        clean_timestamp
                                    );
                                }
                                Err(e) => {
                                    eprintln!("Error sending seek command: {:?}", e);
                                }
                            }
                        } else {
                            eprintln!(
                                "Failed to convert timestamp: {} to seconds",
                                clean_timestamp
                            );
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

fn col_frequency(_ctx: &egui::Context, row: &mut TableRow, term: &Term, _app: &YomineApp) {
    row.col(|ui| {
        if let Some(&freq) = term.frequencies.get("HARMONIC") {
            ui.label(if freq == u32::MAX { "？".to_string() } else { freq.to_string() });
        }
    });
}

fn col_pos(_ctx: &egui::Context, row: &mut TableRow, term: &Term, _app: &YomineApp) {
    let pos = term.part_of_speech.to_string();

    row.col(|ui| {
        ui.label(pos);
    });
}

pub fn term_table(ctx: &egui::Context, app: &mut YomineApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.terms.is_empty() && !app.message_overlay.active {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);

                ui.label(egui::RichText::new("No File Loaded").size(32.0).color(app.theme.cyan()));

                ui.add_space(1.0);

                ui.label(
                    egui::RichText::new("ファイルがまだ読み込まれていません")
                        .size(18.0)
                        .color(app.theme.orange()),
                );

                ui.add_space(10.0);

                let label = egui::Label::new(
                    egui::RichText::new("Open New File")
                        .size(14.0)
                        .color(ctx.style().visuals.weak_text_color()),
                )
                .sense(egui::Sense::click());

                let mut response = ui.add(label);

                if response.hovered() {
                    response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
                if response.clicked() {
                    app.file_modal.open_dialog();
                }
            });
        } else if !app.terms.is_empty() {
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
                    .header(25.0, |header| {
                        header_cols(ctx, header, app);
                    })
                    .body(|body| {
                        let row_height = |i: usize| {
                            let t = &app.terms[i];

                            if let None = t.sentence_references.get(0) {
                                return 36.0;
                            }

                            let sentence = t.sentence_references.get(0).unwrap();
                            let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
                            let lines: Vec<&str> =
                                sentence_content.text.trim().split("\n").collect();
                            (36.0_f32).max(18.0 * (lines.len() as f32)) //Size 22.0 font is not 22 height..
                        };

                        body.heterogeneous_rows(
                            (0..app.terms.iter().len()).map(row_height),
                            |mut row| {
                                let t = &app.terms[row.index()];
                                col_term(ctx, &mut row, t, app);
                                col_sentence(ctx, &mut row, t, app);
                                col_timestamp(ctx, &mut row, t, app);
                                col_frequency(ctx, &mut row, t, app);
                                col_pos(ctx, &mut row, t, app);
                            },
                        );
                    });
            });
        }
    });
}

pub fn header_cols(_ctx: &egui::Context, mut header: TableRow<'_, '_>, app: &mut YomineApp) {
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
                    app.terms.sort_unstable_by(|a, b| {
                        match app.table_state.sort {
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
