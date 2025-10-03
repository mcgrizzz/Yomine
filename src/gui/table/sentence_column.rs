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
use wana_kana::ConvertJapanese;

use crate::{
    core::{
        models::TimeStamp,
        Term,
    },
    gui::{
        theme::blend_colors,
        YomineApp,
    },
    segmentation::word::POS,
};

//const ROW_HEIGHT: f32 = 54.0;
const ROW_SPACING: f32 = 2.0;
const BUTTON_SIZE: f32 = 18.0;

pub(crate) fn col_sentence(
    ctx: &Context,
    row: &mut TableRow,
    term: &Term,
    app: &mut YomineApp,
    term_index: usize,
) {
    row.col(|ui| {
        if term.sentence_references.is_empty() {
            return;
        }

        ui.style_mut().spacing.item_spacing.y = ROW_SPACING;
        // ui.ctx().style_mut(|style| {
        //     style.debug.debug_on_hover = true; // Hover to see widget rectangles and details
        // });
        ui.vertical(|ui| {
            ui.set_max_height(32.0);
            ui.horizontal_centered(|ui| {
                ui_sentence_content(ctx, ui, term, app, term_index);
            });

            ui.horizontal(|ui| {
                //TODO: Nice layout where sentence nav is below sentence content in the row.
                ui_sentence_navigation(ui, term, term_index, app);
                ui_timestamp(ui, term, app, term_index);

                // ui_sentence_content(ctx, ui, term, app, term_index);
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
    app: &YomineApp,
    term_index: usize,
) {
    let sentence_idx = app.table_state.get_sentence_index(term_index);
    let sentence_ref = &term.sentence_references[sentence_idx];

    let sentence_content = match app.sentences.get(sentence_ref.0 as usize) {
        Some(content) => content,
        None => return,
    };

    let surface_index = sentence_ref.1;
    let highlighted_color = app.theme.red(ctx);
    let normal_color = ctx.style().visuals.widgets.noninteractive.fg_stroke.color;
    let highlight_color = ctx.style().visuals.widgets.noninteractive.bg_stroke.color;
    let is_expression = matches!(term.part_of_speech, POS::Expression | POS::NounExpression);

    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(false), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let term_text = if is_expression { &term.full_segment } else { &term.surface_form };

        let mut segments_to_highlight = Vec::new();
        if is_expression {
            segments_to_highlight = find_expression_segments(
                term_text,
                &sentence_content.segments,
                &sentence_content.text,
            );
        }

        // Iterate over segments (same as existing col_sentence logic)
        for (idx, (reading, pos, start, stop)) in sentence_content.segments.iter().enumerate() {
            let segment_text = &sentence_content.text[*start..*stop];

            let is_term = is_segment_part_of_term(
                idx,
                *start,
                *stop,
                is_expression,
                &segments_to_highlight,
                surface_index,
                term_text,
            );

            let color = if is_term {
                blend_colors(highlight_color, highlighted_color, 0.85)
            } else {
                let pos_color = app.theme.pos_color(pos, ctx, normal_color);
                // Heavily mute non-term colors by blending mostly with normal color
                blend_colors(normal_color, pos_color, 0.75)
            };

            let text_color = if is_term {
                blend_colors(highlight_color, highlighted_color, 0.85)
            } else {
                // Non-term text is even more muted
                blend_colors(normal_color, color, 0.75)
            };

            let hover_text = match reading.as_str() {
                "*" => None,
                _ => Some(RichText::new(&format!("{}", reading.to_hiragana())).color(color)),
            };

            let label = egui::Label::new(RichText::new(segment_text).color(text_color).size(16.0));
            let response = ui.add(label);

            if let Some(hover_text) = hover_text {
                response.on_hover_text(hover_text);
            }
        }
    });
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
