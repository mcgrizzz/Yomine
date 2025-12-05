use eframe::egui::{
    self,
    RichText,
    Shape,
    Stroke,
    Ui,
};
use egui_extras::{
    Column,
    TableBuilder,
};
use wana_kana::ConvertJapanese;

use super::{
    theme::blend_colors,
    ActionQueue,
    UiAction,
    YomineApp,
};
use crate::core::Term;

mod filter;
mod header;
mod search;
mod sentence_column;
mod sentence_widget;
pub mod sort;
mod state;

use header::{
    ui_controls_row,
    ui_header_cols,
};
use sentence_column::ui_col_sentence;
pub use sort::{
    SortDirection,
    SortField,
};
pub use state::TableState;

pub fn term_table(ctx: &egui::Context, app: &mut YomineApp) {
    let mut actions = ActionQueue::new();

    egui::CentralPanel::default().show(ctx, |ui| {
        let has_file_data = app.file_data.as_ref().map_or(false, |fd| fd.has_terms());

        if !has_file_data && !app.message_overlay.active {
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
                    app.modals.file.open_dialog();
                }
            });
        } else if has_file_data {
            let freq_manager_handle =
                app.language_tools.as_ref().map(|tools| tools.frequency_manager.clone());
            let freq_manager = freq_manager_handle.as_deref();

            // Access file_data for initialization - drop reference immediately
            {
                let file_data = app.file_data.as_ref().unwrap();
                app.table_state.ensure_indices(
                    &file_data.terms,
                    &file_data.sentences,
                    freq_manager,
                );
            }

            // Access title in its own scope
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(ui.available_width());
                let title = &app.file_data.as_ref().unwrap().source_file.title;
                ui.heading(egui::RichText::new(title).color(app.theme.cyan(ui.ctx())).strong());
            });

            {
                let file_data = app.file_data.as_ref().unwrap();
                app.table_state.compute_term_column_width(ctx, &file_data.terms);
            }

            app.table_state.sync_frequency_states(freq_manager);
            ui_controls_row(ui, app, &mut actions);
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
                let visible_indices = app.table_state.visible_indices().to_vec();
                let available_width = ui.available_width();

                // Calculate sentence column width - subtract term, frequency, and POS columns
                // Frequency and POS are at least 90 points each plus spacing
                let sentence_column_width =
                    (available_width - term_width - (90.0 * 2.0 + 20.0)).max(200.0);

                // Compute row heights - access file_data in scope
                {
                    let file_data = app.file_data.as_ref().unwrap();
                    app.table_state.compute_row_heights(
                        ctx,
                        &file_data.terms,
                        &file_data.sentences,
                        sentence_column_width,
                    );
                }
                let row_heights: Vec<f32> = app.table_state.row_heights().to_vec();

                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(term_width))
                    .column(Column::exact(sentence_column_width))
                    .column(Column::auto().at_least(90.0))
                    .column(Column::auto().at_least(90.0))
                    .header(25.0, |header| {
                        ui_header_cols(ctx, header, app, &mut actions);
                    })
                    .body(|body| {
                        let file_data = app.file_data.as_ref().unwrap();
                        let terms = &file_data.terms;

                        body.heterogeneous_rows(row_heights.iter().copied(), |mut row| {
                            let term_index = visible_indices[row.index()];
                            let term = &terms[term_index];

                            ui_col_term(ctx, &mut row, term, app, &mut actions);
                            ui_col_sentence(ctx, &mut row, term, app, term_index, &mut actions);
                            ui_col_frequency(ctx, &mut row, term, app);
                            ui_col_pos(ctx, &mut row, term, app);
                        });
                    });
            });
        }
    });

    // Execute all queued actions and repaint if needed
    let had_actions = !actions.is_empty();
    execute_actions(app, &mut actions);

    if had_actions {
        ctx.request_repaint();
    }
}

fn execute_actions(app: &mut YomineApp, actions: &mut ActionQueue) {
    for action in actions.drain() {
        match action {
            UiAction::SetSort { field, direction } => {
                app.table_state.set_sort(field, direction);
            }
            UiAction::NextSentence { term_index, total_sentences } => {
                app.table_state.next_sentence(term_index, total_sentences);
            }
            UiAction::PrevSentence { term_index, total_sentences } => {
                app.table_state.prev_sentence(term_index, total_sentences);
            }
            UiAction::SetFrequencyRange { min, max } => {
                app.table_state.set_frequency_range(min, max);
            }
            UiAction::SetIncludeUnknown(include) => {
                app.table_state.set_include_unknown(include);
            }
            UiAction::SetSearch(search) => {
                app.table_state.set_search(search);
            }
            UiAction::OpenPosFilters => {
                app.modals.pos_filters.open_modal(app.table_state.pos_snapshot());
            }
            UiAction::OpenFrequencyWeights => {
                let freq_manager =
                    app.language_tools.as_ref().map(|tools| tools.frequency_manager.as_ref());
                app.modals.frequency_weights.open_modal(&app.settings_data, freq_manager);
            }
            UiAction::AddToIgnoreList(term) => {
                if let Some(ref language_tools) = app.language_tools {
                    if let Ok(mut ignore_list) = language_tools.ignore_list.lock() {
                        let _ = ignore_list.add_term(&term);
                    }
                }
            }
            UiAction::RemoveFromIgnoreList(term) => {
                if let Some(ref language_tools) = app.language_tools {
                    if let Ok(mut ignore_list) = language_tools.ignore_list.lock() {
                        let _ = ignore_list.remove_term(&term);
                    }
                }
            }
            UiAction::SeekTimestamp { seconds, label } => {
                if let Err(e) = app.player.seek_timestamp(seconds, &label) {
                    eprintln!("Failed to seek timestamp: {}", e);
                }
            }
        }
    }
}

fn ui_col_term(
    ctx: &egui::Context,
    row: &mut egui_extras::TableRow,
    term: &Term,
    app: &YomineApp,
    actions: &mut ActionQueue,
) {
    row.col(|ui| {
        ui_col_lines(ui, ctx, app);

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

        let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);

        let label = egui::Label::new(RichText::new(&term.lemma_form).color(term_color).size(22.0))
            .sense(egui::Sense::click())
            .selectable(!ctrl_held);

        let response = ui
            .add(label)
            .on_hover_ui(|ui| {
                ui.set_min_width(75.0);
                ui.label(app.theme.heading(ui.ctx(), &term.lemma_reading));

                ui.separator();
                ui.add_space(4.0);

                let mut text = "Ctrl+Click to ignore";

                if ignore_status {
                    text = "Ctrl+Click to UNDO ignore";
                }

                ui.label(
                    egui::RichText::new(text)
                        .color(ctx.style().visuals.weak_text_color())
                        .size(10.0)
                        .italics(),
                );
            });

        if response.clicked() && ctrl_held {
            if ignore_status {
                actions.push(UiAction::RemoveFromIgnoreList(term.lemma_form.clone()));
            } else {
                actions.push(UiAction::AddToIgnoreList(term.lemma_form.clone()));
            }
        }

        if response.hovered() && ctrl_held {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response.context_menu(|ui| {
            if app.language_tools.is_some() {
                if ignore_status {
                    if ui.button("Remove from ignore list").clicked() {
                        actions.push(UiAction::RemoveFromIgnoreList(term.lemma_form.clone()));
                        ui.close();
                    }
                } else if ui.button("Add to ignore list").clicked() {
                    actions.push(UiAction::AddToIgnoreList(term.lemma_form.clone()));
                    ui.close();
                }
            } else {
                ui.label("Language tools not loaded");
            }
        });
    });
}

pub(crate) fn ui_col_lines(ui: &mut Ui, ctx: &egui::Context, app: &YomineApp) {
    let mut color = app.theme.comment(ctx);
    color = color.linear_multiply(0.55);

    let st = Stroke { width: 0.5, color };

    let rect = ui.max_rect();
    let xr = rect.x_range();
    let yr = rect.y_range();

    let shape = Shape::dashed_line(
        &[egui::pos2(xr.min, yr.min), egui::pos2(xr.max, yr.min)],
        st,
        5.0, // dash length
        2.5, // gap length
    );
    ui.painter().add(shape);
}

fn ui_col_frequency(
    ctx: &egui::Context,
    row: &mut egui_extras::TableRow,
    term: &Term,
    app: &YomineApp,
) {
    row.col(|ui| {
        ui_col_lines(ui, ctx, app);

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

fn ui_col_pos(ctx: &egui::Context, row: &mut egui_extras::TableRow, term: &Term, app: &YomineApp) {
    row.col(|ui| {
        ui_col_lines(ui, ctx, app);
        ui.label(term.part_of_speech.to_string());
    });
}
