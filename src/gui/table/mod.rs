use eframe::egui::{
    self,
    RichText,
};
use egui_extras::{
    Column,
    TableBuilder,
};
use wana_kana::ConvertJapanese;

use super::{
    theme::blend_colors,
    YomineApp,
};
use crate::core::Term;

mod filter;
mod header;
mod search;
mod sentence_column;
mod sentence_widget;
mod sort;
mod state;

use header::{
    ui_controls_row,
    ui_header_cols,
};
use sentence_column::ui_col_sentence;
pub use state::TableState;

pub fn term_table(ctx: &egui::Context, app: &mut YomineApp) {
    egui::CentralPanel::default().show(ctx, |ui| {
        if app.terms.is_empty() && !app.message_overlay.active {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);

                ui.label(
                    egui::RichText::new("No File Loaded")
                        .size(32.0)
                        .color(app.theme.cyan(ui.ctx())),
                );

                ui.add_space(1.0);

                ui.label(
                    egui::RichText::new("ファイルがまだ読み込まれていません")
                        .size(18.0)
                        .color(app.theme.orange(ui.ctx())),
                );

                ui.add_space(1.0);

                ui.label(
                    egui::RichText::new("ℹ You can drag and drop a file at any time to load it.")
                        .size(10.0)
                        .color(app.theme.comment(ui.ctx())),
                );

                ui.add_space(16.0);
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
            let freq_manager_handle =
                app.language_tools.as_ref().map(|tools| tools.frequency_manager.clone());
            let freq_manager = freq_manager_handle.as_deref();
            app.table_state.ensure_indices(&app.terms, &app.sentences, freq_manager);

            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(ui.available_width());
                if let Some(ref source_file) = app.current_source_file {
                    ui.heading(
                        egui::RichText::new(&source_file.title)
                            .color(app.theme.cyan(ui.ctx()))
                            .strong(),
                    );
                } else {
                    ui.heading("Term Table");
                }
            });

            app.table_state.sync_frequency_states(freq_manager);
            app.table_state.compute_term_column_width(ctx, &app.terms);
            ui_controls_row(ui, app);
            ui.add_space(10.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Enhance row background contrast for better text readability
                let base_bg = ui.visuals().faint_bg_color;
                ui.style_mut().visuals.faint_bg_color = if ui.visuals().dark_mode {
                    base_bg.linear_multiply(1.4) // Make stripes slightly lighter in dark mode
                } else {
                    base_bg.linear_multiply(0.75) // Make stripes slightly darker in light mode
                };

                let term_width = app.table_state.term_column_width();

                // Pre-calculate everything we need before entering closures
                let visible_indices = app.table_state.visible_indices().to_vec();
                let fonts = ctx.fonts(|f| f.clone());
                let font_id = egui::FontId::proportional(16.0);
                let available_width = ui.available_width();

                // Calculate sentence column width - subtract term, frequency, and POS columns
                // Frequency and POS are at least 90 points each plus spacing
                let sentence_column_width =
                    (available_width - term_width - (90.0 * 2.0 + 20.0)).max(200.0);

                // Pre-calculate all row heights
                let row_heights: Vec<f32> = visible_indices
                    .iter()
                    .map(|&term_index| {
                        let term = &app.terms[term_index];

                        if term.sentence_references.is_empty() {
                            46.0
                        } else {
                            let sentence_idx = app.table_state.get_sentence_index(term_index);
                            if let Some(sentence_ref) = term.sentence_references.get(sentence_idx) {
                                if let Some(sentence) = app.sentences.get(sentence_ref.0) {
                                    let mut layout_job = egui::text::LayoutJob::default();
                                    layout_job.wrap = egui::text::TextWrapping {
                                        max_width: sentence_column_width,
                                        max_rows: 10,
                                        break_anywhere: false,
                                        overflow_character: None,
                                    };
                                    layout_job.append(
                                        &sentence.text,
                                        0.0,
                                        egui::TextFormat {
                                            font_id: font_id.clone(),
                                            color: egui::Color32::WHITE,
                                            ..Default::default()
                                        },
                                    );

                                    let galley = fonts.layout_job(layout_job);
                                    let lines = galley.rows.len() as f32;

                                    // Height = text height + controls height + spacing
                                    // Measured values from egui debug mode:
                                    // Text: 22.9 points per line
                                    // 2.05 between line spacing
                                    // Controls: 18.2 points
                                    let text_height = lines * 22.9 + 2.05 * (lines - 1.0);
                                    let controls_height = 18.2;
                                    let spacing = 2.1; // spacing between text and controls
                                    text_height + controls_height + spacing
                                } else {
                                    46.0
                                }
                            } else {
                                46.0
                            }
                        }
                    })
                    .collect();

                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(term_width))
                    .column(Column::exact(sentence_column_width))
                    .column(Column::auto().at_least(90.0))
                    .column(Column::auto().at_least(90.0))
                    .header(25.0, |header| {
                        ui_header_cols(ctx, header, app);
                    })
                    .body(|body| {
                        body.heterogeneous_rows(row_heights.iter().copied(), |mut row| {
                            let term_index = visible_indices[row.index()];
                            let term = app.terms[term_index].clone();

                            ui_col_term(ctx, &mut row, &term, app);
                            ui_col_sentence(ctx, &mut row, &term, app, term_index);
                            ui_col_frequency(ctx, &mut row, &term, app);
                            ui_col_pos(ctx, &mut row, &term);
                        });
                    });
            });
        }
    });
}

fn ui_col_term(
    ctx: &egui::Context,
    row: &mut egui_extras::TableRow,
    term: &Term,
    app: &mut YomineApp,
) {
    row.col(|ui| {
        let ignore_status = if let Some(ref language_tools) = app.language_tools {
            language_tools
                .ignore_list
                .lock()
                .map(|ignore_list| ignore_list.contains(&term.lemma_form))
                .unwrap_or(false)
        } else {
            false
        };

        let term_color = if ignore_status {
            ctx.style().visuals.weak_text_color()
        } else {
            let highlighted_color = app.theme.red(ctx);
            let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;
            blend_colors(normal_color, highlighted_color, 0.8)
        };

        let response = ui
            .label(RichText::new(&term.lemma_form).color(term_color).size(22.0))
            .on_hover_ui(|ui| {
                ui.set_min_width(75.0);
                ui.label(app.theme.heading(ui.ctx(), &term.lemma_reading.to_hiragana()));
                ui.label(app.theme.heading(ui.ctx(), &term.lemma_reading.to_katakana()));

                ui.separator();
                if ignore_status {
                    ui.label(
                        egui::RichText::new("This term is ignored")
                            .color(ctx.style().visuals.weak_text_color())
                            .size(12.0),
                    );
                }
            });

        response.context_menu(|ui| {
            if let Some(ref language_tools) = app.language_tools {
                if ignore_status {
                    if ui.button("Remove from ignore list").clicked() {
                        if let Ok(mut ignore_list) = language_tools.ignore_list.lock() {
                            if let Err(e) = ignore_list.remove_term(&term.lemma_form) {
                                eprintln!("Failed to remove term from ignore list: {}", e);
                            }
                        }
                        ui.close();
                    }
                } else if ui.button("Add to ignore list").clicked() {
                    if let Ok(mut ignore_list) = language_tools.ignore_list.lock() {
                        if let Err(e) = ignore_list.add_term(&term.lemma_form) {
                            eprintln!("Failed to add term to ignore list: {}", e);
                        }
                    }
                    ui.close();
                }
            } else {
                ui.label("Language tools not loaded");
            }
        });
    });
}

fn ui_col_frequency(
    _ctx: &egui::Context,
    row: &mut egui_extras::TableRow,
    term: &Term,
    app: &YomineApp,
) {
    row.col(|ui| {
        let weighted = if let Some(manager) =
            app.language_tools.as_ref().map(|tools| tools.frequency_manager.as_ref())
        {
            manager.get_weighted_harmonic(&term.frequencies)
        } else {
            term.frequencies.get("HARMONIC").copied().unwrap_or(u32::MAX)
        };

        let display = if weighted == u32::MAX { "？".to_string() } else { weighted.to_string() };
        ui.label(display);
    });
}

fn ui_col_pos(_ctx: &egui::Context, row: &mut egui_extras::TableRow, term: &Term) {
    row.col(|ui| {
        ui.label(term.part_of_speech.to_string());
    });
}
