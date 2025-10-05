use eframe::egui::{
    self,
    Atom,
    AtomLayout,
    Context,
    RichText,
    Ui,
    Widget,
};
use egui_extras::TableRow;

use super::sentence_widget::SentenceWidget;
use crate::{
    core::{
        models::TimeStamp,
        Term,
    },
    gui::YomineApp,
};

//const ROW_HEIGHT: f32 = 54.0;
const ROW_SPACING: f32 = 2.0;
const BUTTON_SIZE: f32 = 18.0;

pub(crate) fn ui_col_sentence(
    ctx: &Context,
    row: &mut TableRow,
    term: &Term,
    app: &mut YomineApp,
    term_index: usize,
) {
    row.col(|ui| {
        super::ui_col_lines(ui, ctx, app);

        if term.sentence_references.is_empty() {
            return;
        }

        ui.style_mut().spacing.item_spacing.y = ROW_SPACING;

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui_sentence_content(ctx, ui, term, app, term_index);
            });

            ui.horizontal(|ui| {
                //TODO: Nice layout where sentence nav is below sentence content in the row.
                ui_sentence_navigation(ui, term, term_index, app);
                ui_timestamp(ui, term, app, term_index);
            });
        });
    });
}

fn ui_timestamp(ui: &mut Ui, term: &Term, app: &YomineApp, term_index: usize) {
    let sentence_idx = app.table_state.get_sentence_index(term_index);
    let sentence_ref = &term.sentence_references[sentence_idx];

    let sentence_content = match app.sentences.get(sentence_ref.0 as usize) {
        Some(content) => content,
        None => return,
    };

    if let Some(timestamp) = &sentence_content.timestamp {
        let player_available = app.player.is_connected();
        if player_available {
            ui_timestamp_button(ui, timestamp, app);
        } else {
            let (human_timestamp_start, _human_timestamp_stop) = timestamp.to_human_readable();
            ui.label(
                RichText::new(&human_timestamp_start)
                    .color(ui.ctx().style().visuals.weak_text_color())
                    .size(11.0),
            );
        }
    }
}

fn ui_sentence_navigation(ui: &mut Ui, term: &Term, term_index: usize, app: &mut YomineApp) {
    let sentence_count = term.sentence_references.len();
    let current_index = app.table_state.get_sentence_index(term_index);

    ui.horizontal(|ui| {
        //let prev_atom = Atom::from("⏮").atom_size(Vec2::splat(BUTTON_SIZE));
        let prev_button = egui::Button::new("⏮").corner_radius(egui::CornerRadius::same(2)).small();

        if ui.add_enabled(sentence_count > 1, prev_button).clicked() {
            app.table_state.prev_sentence(term_index, sentence_count);
            ui.ctx().request_repaint();
        }

        ui.allocate_ui_with_layout(
            egui::Vec2::new(BUTTON_SIZE * 2.0, BUTTON_SIZE),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                let counter_text = format!("{}/{}", current_index + 1, sentence_count);
                let counter_atom = Atom::from(
                    RichText::new(counter_text).size(11.0).color(app.theme.cyan(ui.ctx())),
                );
                ui.add(AtomLayout::new(counter_atom));
            },
        );

        //let next_atom = Atom::from("⏭").atom_size(Vec2::splat(BUTTON_SIZE));
        let next_button = egui::Button::new("⏭").corner_radius(egui::CornerRadius::same(2)).small();

        if ui.add_enabled(sentence_count > 1, next_button).clicked() {
            app.table_state.next_sentence(term_index, sentence_count);
            ui.ctx().request_repaint();
        }
    });
}

fn ui_sentence_content(
    ctx: &Context,
    ui: &mut Ui,
    term: &Term,
    app: &mut YomineApp,
    term_index: usize,
) {
    let sentence_idx = app.table_state.get_sentence_index(term_index);
    let sentence_ref = &term.sentence_references[sentence_idx];

    let sentence_content = match app.sentences.get(sentence_ref.0 as usize) {
        Some(content) => content,
        None => return,
    };

    let surface_index = sentence_ref.1;

    // Use the new SentenceWidget for consistent wrapping
    let widget = SentenceWidget::new(
        ctx,
        term,
        app,
        &sentence_content.text,
        &sentence_content.segments,
        surface_index,
    );

    ui.add(widget);
}

/// Creates a clickable timestamp button with WebSocket integration
fn ui_timestamp_button(ui: &mut Ui, timestamp: &TimeStamp, app: &YomineApp) {
    let (seconds_start, _seconds_stop) = timestamp.to_secs();
    let (human_timestamp_start, _human_timestamp_stop) = timestamp.to_human_readable();

    let is_confirmed = app.player.get_confirmed_timestamps().contains(&seconds_start);

    // Color based on confirmation status
    let button_text = if is_confirmed {
        format!("👁 {}", human_timestamp_start) // Eye for confirmed
    } else {
        format!("▶ {}", human_timestamp_start) // Play button for not confirmed
    };

    ui.horizontal_centered(|ui| {
        //let button_atom = Atom::from(button_text).atom_size(Vec2::new(60.0, BUTTON_SIZE));
        let mut button = egui::Button::new(button_text).small();

        let button_color = egui::Color32::from_hex("#559449ff");
        if is_confirmed {
            button = button.fill(button_color.clone().unwrap());
        }

        // let outline = blend_colors(button_color.unwrap(), app.theme.highlight(ui.ctx()), 0.8);
        // button = button.stroke(egui::Stroke::new(1.0, outline));

        let response = button.ui(ui);

        if response.clicked() {
            if let Err(e) = app.player.seek_timestamp(seconds_start, &human_timestamp_start) {
                eprintln!("Failed to seek timestamp: {}", e);
            }
            println!("Sent seek command for timestamp: {}", &human_timestamp_start);
        }
    });
}
