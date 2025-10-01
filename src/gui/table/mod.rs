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

mod header;
mod sentence_column;
mod state;

use header::{
    controls_row,
    header_cols,
};
use sentence_column::col_sentence;
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

            if let Some(ref source_file) = app.current_source_file {
                ui.heading(
                    egui::RichText::new(&source_file.title)
                        .color(app.theme.cyan(ui.ctx()))
                        .strong(),
                );
            } else {
                ui.heading("Term Table");
            }

            app.table_state.sync_frequency_states(freq_manager);
            controls_row(ui, app);
            ui.add_space(10.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                TableBuilder::new(ui)
                    .striped(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::auto().at_least(100.0))
                    .column(Column::remainder())
                    .column(Column::auto().at_least(40.0))
                    .column(Column::auto().at_least(40.0))
                    .header(25.0, |header| {
                        header_cols(ctx, header, app);
                    })
                    .body(|body| {
                        let visible_indices = app.table_state.visible_indices().to_vec();
                        let row_count = visible_indices.len();

                        body.rows(53.7, row_count, |mut row| {
                            let visible_index = row.index();
                            let term_index = visible_indices
                                .get(visible_index)
                                .copied()
                                .unwrap_or(visible_index);
                            let term = app.terms[term_index].clone();
                            col_term(ctx, &mut row, &term, app);
                            col_sentence(ctx, &mut row, &term, app, term_index);
                            col_frequency(ctx, &mut row, &term, app);
                            col_pos(ctx, &mut row, &term);
                        });
                    });
            });
        }
    });
}

fn col_term(
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

fn col_frequency(
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

fn col_pos(_ctx: &egui::Context, row: &mut egui_extras::TableRow, term: &Term) {
    row.col(|ui| {
        ui.label(term.part_of_speech.to_string());
    });
}
