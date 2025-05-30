use eframe::egui::{
    self,
    Color32,
    Context,
    Response,
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
    row.col(|ui| {
        let highlighted_color = app.theme.red();
        let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;
        let term_color = blend_colors(normal_color, highlighted_color, 0.8);

        ui.label(RichText::new(&term.lemma_form).color(term_color).size(22.0))
            .on_hover_ui_at_pointer(|ui| {
                ui.label(app.theme.heading(&term.lemma_reading.to_hiragana()));
                ui.label(app.theme.heading(&term.lemma_reading.to_katakana()));
            });
    });
}

/// Determines the color for a part of speech
fn get_pos_color(pos: &POS, normal_color: Color32, app: &YomineApp) -> Color32 {
    match pos {
        POS::Verb | POS::SuruVerb => blend_colors(normal_color, app.theme.blue(), 0.75),
        POS::Noun => blend_colors(normal_color, app.theme.green(), 0.75),
        POS::Adjective | POS::AdjectivalNoun => {
            blend_colors(normal_color, app.theme.orange(), 0.75)
        }
        POS::Adverb => blend_colors(normal_color, app.theme.purple(), 0.75),
        POS::Postposition => blend_colors(normal_color, Color32::BLACK, 0.25),
        _ => normal_color,
    }
}

/// Finds which segments form an expression by matching consecutive segments
fn find_expression_segments(
    term_text: &str,
    sentence_segments: &[(String, POS, usize, usize)],
    sentence_text: &str,
) -> Vec<usize> {
    let mut segments_to_highlight = Vec::new();
    let mut current_text = String::new();
    let mut start_idx = None;

    for (idx, (_, _, start, stop)) in sentence_segments.iter().enumerate() {
        let segment_text = &sentence_text[*start..*stop];
        let potential_text = current_text.clone() + segment_text;

        if term_text.starts_with(&potential_text) {
            // This segment could be part of the expression
            if start_idx.is_none() {
                start_idx = Some(idx);
            }
            current_text = potential_text;

            if current_text == *term_text {
                // We found the complete expression
                if let Some(start) = start_idx {
                    for i in start..=idx {
                        segments_to_highlight.push(i);
                    }
                }
                break;
            }
        } else {
            // Reset if this segment doesn't continue the pattern
            current_text.clear();
            start_idx = None;

            // Check if this single segment starts the expression
            if term_text.starts_with(segment_text) {
                current_text = segment_text.to_string();
                start_idx = Some(idx);

                if current_text == *term_text {
                    segments_to_highlight.push(idx);
                    break;
                }
            }
        }
    }

    segments_to_highlight
}

/// Determines if a segment belongs to a term
fn is_segment_part_of_term(
    idx: usize,
    start: usize,
    stop: usize,
    is_expression: bool,
    expression_segments: &[usize],
    surface_index: usize,
    term_text: &str,
) -> bool {
    if is_expression {
        // For expressions, check if this segment index is in our highlight list
        expression_segments.contains(&idx)
    } else {
        // For regular terms, use character position overlap with surface_index
        let term_start = surface_index;
        let term_end = term_start + term_text.len();
        start < term_end && stop > term_start
    }
}

fn segment_ui(
    ui: &mut Ui,
    text: &str,
    text_color: Color32,
    _underline_color: Option<Color32>, // Don't use underlines in wrapping context
    _highlight: bool,
    hover_text: Option<RichText>,
    _skip_underline: bool,
) -> Response {
    let label = egui::Label::new(RichText::new(text).color(text_color));
    let response = ui.add(label);

    // Don't draw underlines in wrapping context as they get misaligned
    // This will be handled at a higher level if needed

    if let Some(hover_text) = hover_text {
        response.on_hover_text(hover_text)
    } else {
        response
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
        let is_expression = matches!(term.part_of_speech, POS::Expression | POS::NounExpression);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // For expressions, we need to match the exact text content
            let term_text = if is_expression { &term.full_segment } else { &term.surface_form };

            // For expressions, find consecutive segments that form the expression
            let mut segments_to_highlight = Vec::new();
            if is_expression {
                segments_to_highlight = find_expression_segments(
                    term_text,
                    &sentence_content.segments,
                    &sentence_content.text,
                );
            }

            // Iterate over segments
            for (idx, (reading, pos, start, stop)) in sentence_content.segments.iter().enumerate() {
                let segment_text = &sentence_content.text[*start..*stop];

                // Check if this segment is part of the term
                let is_term = is_segment_part_of_term(
                    idx,
                    *start,
                    *stop,
                    is_expression,
                    &segments_to_highlight,
                    surface_index,
                    term_text,
                );

                // Determine colors
                let color = if is_term {
                    blend_colors(normal_color, highlighted_color, 0.95)
                } else {
                    get_pos_color(pos, normal_color, app)
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

                // For expressions, skip individual underlines; for symbols, don't underline
                let (underline_color, skip_underline) = match pos {
                    POS::Symbol => (None, false),
                    _ => {
                        if is_expression && is_term {
                            (Some(color), true) // Skip individual underlines for expressions
                        } else {
                            (Some(color), false) // Normal underlines for non-expressions
                        }
                    }
                };

                // Use segment_ui to add the segment
                segment_ui(
                    ui,
                    segment_text,
                    text_color,
                    underline_color,
                    is_term,
                    hover_text,
                    skip_underline,
                );
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

fn col_timestamp(
    _ctx: &egui::Context,
    row: &mut TableRow,
    term: &Term,
    app: &YomineApp,
    has_websocket_clients: bool,
) {
    row.col(|ui| {
        if let None = term.sentence_references.get(0) {
            return;
        }

        let sentence = term.sentence_references.get(0).unwrap();
        let sentence_content = app.sentences.get(sentence.0 as usize).unwrap();
        if let Some(timestamp) = &sentence_content.timestamp {
            if has_websocket_clients {
                // If we have connected clients, show a clickable button
                create_timestamp_button(ui, timestamp, app);
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
            // Check websocket state once for all terms
            let has_websocket_clients =
                app.websocket_manager.has_clients() && app.websocket_manager.server.is_some();

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
                                col_timestamp(ctx, &mut row, t, app, has_websocket_clients);
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

/// Creates a clickable timestamp button with WebSocket integration
fn create_timestamp_button(ui: &mut Ui, timestamp: &str, app: &YomineApp) {
    // Extract the first part of the timestamp if it contains an arrow
    let clean_timestamp = timestamp.split(" --> ").next().unwrap_or(timestamp);

    // Format the timestamp in a more human-readable way
    let human_timestamp = format_human_timestamp(clean_timestamp);

    let is_confirmed =
        app.websocket_manager.get_confirmed_timestamps().contains(&clean_timestamp.to_string());

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

    if response.clicked() {
        if let Some(server) = &app.websocket_manager.server {
            if let Ok(seconds) =
                crate::websocket::WebSocketServer::convert_srt_timestamp_to_seconds(clean_timestamp)
            {
                // Send the timestamp to all connected clients
                match server.seek_timestamp(seconds, clean_timestamp) {
                    Ok(_) => {
                        println!("Sent seek command for timestamp: {}", clean_timestamp);
                    }
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
}
