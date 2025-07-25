use std::collections::HashMap;

use eframe::egui::{
    self,
    RichText,
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
use crate::core::Term;

mod sentence_column;

use sentence_column::col_sentence;

pub struct TableState {
    sort: TableSort,
    sentence_indices: HashMap<usize, usize>, //term_index, sentence_index
}

impl Default for TableState {
    fn default() -> Self {
        Self { sort: TableSort::FrequencyAscending, sentence_indices: HashMap::new() }
    }
}

impl TableState {
    pub fn get_sentence_index(&self, term_index: usize) -> usize {
        self.sentence_indices.get(&term_index).copied().unwrap_or(0)
    }

    pub fn next_sentence(&mut self, term_index: usize, total_sentences: usize) {
        let current = self.get_sentence_index(term_index);
        let next = (current + 1) % total_sentences;
        self.sentence_indices.insert(term_index, next);
    }

    pub fn prev_sentence(&mut self, term_index: usize, total_sentences: usize) {
        let current = self.get_sentence_index(term_index);
        let next = if current == 0 { total_sentences - 1 } else { current - 1 };
        self.sentence_indices.insert(term_index, next);
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

            // Display current file's parsed title as the main heading
            if let Some(ref source_file) = app.current_source_file {
                ui.heading(
                    egui::RichText::new(&source_file.title)
                        .color(app.theme.cyan(ui.ctx()))
                        .strong(),
                );
            } else {
                ui.heading("Term Table");
            }
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
                        // let row_heights: Vec<f32> = app
                        //     .terms
                        //     .iter()
                        //     .map(|t| {
                        //         if t.sentence_references.is_empty() {
                        //             return 36.0;
                        //         }

                        //         let sentence = t.sentence_references.get(0).unwrap();
                        //         let sentence_content =
                        //             app.sentences.get(sentence.0 as usize).unwrap();
                        //         let lines: Vec<&str> =
                        //             sentence_content.text.trim().split("\n").collect();
                        //         (36.0_f32).max(18.0 * (lines.len() as f32)) //Size 22.0 font is not 22 height..
                        //     })
                        //     .collect();

                        body.rows(53.7, app.terms.len(), |mut row| {
                            let term_index = row.index();
                            let term = &app.terms[term_index].clone();
                            col_term(ctx, &mut row, &term, app);
                            col_sentence(
                                ctx,
                                &mut row,
                                &term,
                                app,
                                has_websocket_clients,
                                term_index,
                            );
                            col_frequency(ctx, &mut row, &term, app);
                            col_pos(ctx, &mut row, &term, app);
                        });
                    });
            });
        }
    });
}

fn col_term(ctx: &egui::Context, row: &mut TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        let highlighted_color = app.theme.red(ctx);
        let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;
        let term_color = blend_colors(normal_color, highlighted_color, 0.8);

        ui.label(RichText::new(&term.lemma_form).color(term_color).size(22.0))
            .on_hover_ui_at_pointer(|ui| {
                ui.label(app.theme.heading(ui.ctx(), &term.lemma_reading.to_hiragana()));
                ui.label(app.theme.heading(ui.ctx(), &term.lemma_reading.to_katakana()));
            });
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

pub fn header_cols(_ctx: &egui::Context, mut header: TableRow<'_, '_>, app: &mut YomineApp) {
    header.col(|ui| {
        ui.label(app.theme.heading(ui.ctx(), "Term"));
    });
    header.col(|ui| {
        ui.label(app.theme.heading(ui.ctx(), "Sentence"));
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
                ui.label(app.theme.heading(ui.ctx(), "Frequency"));
            },
        );
    });

    header.col(|ui| {
        ui.label(app.theme.heading(ui.ctx(), "Part of Speech"));
    });
}
