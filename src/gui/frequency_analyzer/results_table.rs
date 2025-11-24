use eframe::egui;
use egui_extras::{
    Column,
    TableBuilder,
};

use crate::tools::analysis::TermEntry;

// Table Constants
const RESULTS_TABLE_HEIGHT: f32 = 300.0;
const TABLE_TERM_COLUMN_WIDTH: f32 = 120.0;
const TABLE_COUNT_COLUMN_WIDTH: f32 = 80.0;
const TABLE_HEADER_HEIGHT: f32 = 20.0;
const TABLE_ROW_HEIGHT: f32 = 18.0;
const SMALL_SPACING: f32 = 4.0;

pub struct ResultsTableWidget;

impl ResultsTableWidget {
    pub fn show(
        ui: &mut egui::Ui,
        entries: &[TermEntry],
        show_top: &mut bool,
        display_limit: usize,
    ) {
        ui.horizontal(|ui| {
            ui.label("Show:");
            ui.radio_value(show_top, true, "Top 250");
            ui.radio_value(show_top, false, "Bottom 250");
        });

        ui.add_space(SMALL_SPACING);

        let filtered_entries: &[TermEntry] = if *show_top {
            let end = display_limit.min(entries.len());
            &entries[..end]
        } else {
            let skip_count = entries.len().saturating_sub(display_limit);
            &entries[skip_count..]
        };

        egui::ScrollArea::vertical().max_height(RESULTS_TABLE_HEIGHT).show(ui, |ui| {
            TableBuilder::new(ui)
                .striped(true)
                .column(Column::initial(TABLE_TERM_COLUMN_WIDTH).resizable(true))
                .column(Column::initial(TABLE_COUNT_COLUMN_WIDTH).resizable(true))
                .header(TABLE_HEADER_HEIGHT, |mut header| {
                    header.col(|ui| {
                        ui.strong("Term");
                    });
                    header.col(|ui| {
                        ui.strong("Count");
                    });
                })
                .body(|mut body| {
                    for entry in filtered_entries {
                        body.row(TABLE_ROW_HEIGHT, |mut row| {
                            row.col(|ui| {
                                ui.label(&entry.term);
                            });
                            row.col(|ui| {
                                ui.label(entry.frequency.to_string());
                            });
                        });
                    }
                });
        });
    }
}
