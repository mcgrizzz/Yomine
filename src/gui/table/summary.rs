use eframe::egui::{
    self,
    Color32,
    RichText,
    Sense,
    Ui,
};

use crate::{
    gui::{
        ActionQueue,
        UiAction,
        YomineApp,
    },
    tools::knowledge_summary::{
        BandStats,
        KnowledgeMode,
    },
};

/// Rendered width of a bar label in the given font.
fn label_width(ui: &Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE).size().x
}

pub fn ui_knowledge_profile(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
    // Skip the card entirely when there's nothing to show, so we don't draw an empty box.
    let Some(summary) = &app.knowledge_summary else {
        return;
    };
    if summary.jlpt.is_empty() && summary.frequency.is_empty() {
        return;
    }

    // egui has no shrink-to-content vertical layout... a top-down column always claims the full
    // available width, which would stretch the framed card across the whole column. The bars are
    // fixed-width with short static labels, so pin the card to the widest row's width. That width
    // only changes if the set of bands does, so its cached
    let width_id =
        egui::Id::new(("knowledge_card_width", summary.jlpt.len(), summary.frequency.len()));
    let card_width = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or_else(|| {
        let font = egui::FontId::monospace(11.0);
        let jlpt_w: f32 = summary
            .jlpt
            .iter()
            .map(|(level, _)| label_width(ui, level.label(), &font) + 3.0 + 40.0)
            .sum::<f32>()
            + 6.0 * summary.jlpt.len().saturating_sub(1) as f32;
        let freq_w: f32 = summary
            .frequency
            .iter()
            .map(|(label, _)| label_width(ui, label, &font) + 3.0 + 28.0)
            .sum::<f32>()
            + 6.0 * summary.frequency.len().saturating_sub(1) as f32;
        let width = jlpt_w.max(freq_w) + 1.0;
        ui.data_mut(|d| d.insert_temp(width_id, width));
        width
    });

    // Right-to-left outer layout pins the card to the right; Align::Max inside right-aligns the
    // rows. Note Align::Max makes child `ui.horizontal`s right-to-left (egui's
    // prefer_right_to_left), which the rows below account for by iterating in reverse.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    ui.set_width(card_width);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui_knowledge_summary(ui, app, actions);
                });
            });
    });
}

/// Current-file comprehension estimate + term counts, rendered left-aligned under the
/// file title (the title itself names the file, so no extra "Current file" header here).
pub fn ui_current_file_summary(ui: &mut Ui, app: &YomineApp) {
    let Some(file_data) = &app.file_data else {
        return;
    };

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;

        // Comprehension estimate — only once we have sentences and fetched Anki data.
        if !file_data.sentences.is_empty() && !file_data.anki_filtered_terms.is_empty() {
            let comp_pct = file_data.file_comprehension * 100.0;

            // Color gradient from red (0%) to yellow to green (100%).
            let base_color = if comp_pct >= 50.0 {
                let t = (comp_pct - 50.0) / 50.0;
                Color32::from_rgb((180.0 * (1.0 - t)) as u8, 180, 60)
            } else {
                let t = comp_pct / 50.0;
                Color32::from_rgb(180, (180.0 * t) as u8, 60)
            };
            let color = base_color.blend(Color32::from_gray(140).gamma_multiply(0.6));

            ui.label(
                RichText::new(format!("Comprehension estimate: {:.1}%", comp_pct))
                    .color(color)
                    .size(13.0)
                    .strong(),
            )
            .on_hover_text("Overall estimated comprehension across all sentences");
        }

        // Term counts: shown / known / total.
        ui.horizontal(|ui| {
            let weak = ui.visuals().weak_text_color();
            let visible_count = app.table_state.visible_indices().len();
            let filtered_count = file_data.terms.len();
            let total_count = file_data.original_terms.len();
            let known_count = total_count.saturating_sub(filtered_count);

            ui.label(RichText::new(format!("{} shown", visible_count)).color(weak).size(12.0));
            ui.label(RichText::new("/").color(weak).size(12.0));

            if known_count > 0 {
                let ignore_count = if let Some(ref language_tools) = app.language_tools {
                    if let Ok(ignore_list) = language_tools.ignore_list.lock() {
                        file_data
                            .original_terms
                            .iter()
                            .filter(|term| ignore_list.contains(&term.lemma_form))
                            .count()
                    } else {
                        0
                    }
                } else {
                    0
                };
                let anki_filtered = known_count.saturating_sub(ignore_count);

                let known_label = ui
                    .label(RichText::new(format!("{} known", known_count)).color(weak).size(12.0));
                known_label.on_hover_ui(|ui| {
                    ui.label(format!("Ignore list: {}", ignore_count));
                    ui.label(format!("Anki filtered: {}", anki_filtered));
                });

                ui.label(RichText::new("/").color(weak).size(12.0));
            }

            ui.label(RichText::new(format!("{} total", total_count)).color(weak).size(12.0));
        });
    });
}

fn ui_mode_header(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
    let mode = app.knowledge_summary_mode;
    let hover = format!("Switch to {}", mode.toggled().title());

    ui.horizontal(|ui| {
        let title = ui
            .add(
                egui::Label::new(
                    RichText::new(mode.title())
                        .color(ui.visuals().text_color())
                        .size(14.0)
                        .strong(),
                )
                .sense(Sense::click()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(hover.as_str());

        let button = ui
            .add(egui::Button::new(RichText::new("⇄").size(13.0)).frame(false).small())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(hover.as_str());

        if button.clicked() || title.clicked() {
            actions.push(UiAction::ToggleKnowledgeMode);
        }
    });
}

fn ui_knowledge_summary(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
    let Some(summary) = &app.knowledge_summary else {
        return;
    };

    ui_mode_header(ui, app, actions);

    let frac = |stats: &BandStats| match app.knowledge_summary_mode {
        KnowledgeMode::Coverage => stats.coverage,
        KnowledgeMode::Estimate => stats.comprehension,
    };

    if !summary.jlpt.is_empty() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (level, stats) in summary.jlpt.iter().rev() {
                coverage_mini_bar(ui, level.label(), stats, frac(stats), 40.0, false);
            }
        });
    }

    if !summary.frequency.is_empty() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (label, stats) in summary.frequency.iter().rev() {
                coverage_mini_bar(ui, label, stats, frac(stats), 28.0, true);
            }
        });
    }
}

fn coverage_mini_bar(
    ui: &mut Ui,
    text: &str,
    stats: &BandStats,
    value: f32,
    bar_width: f32,
    label_in_hover: bool,
) {
    let frac = value.clamp(0.0, 1.0);
    let color = coverage_color(frac * 100.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;

        let bg = ui.visuals().extreme_bg_color;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(bar_width, 9.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, bg);
        if frac > 0.0 {
            let mut fill = rect;
            fill.set_width(rect.width() * frac);
            painter.rect_filled(fill, 2.0, color);
        }

        ui.label(RichText::new(text).monospace().size(11.0).color(ui.visuals().weak_text_color()));

        let comprehended = (frac * stats.total as f32).round() as usize;
        let pct = frac * 100.0;
        let hover = if label_in_hover {
            format!("{} {}/{} {:.0}%", text, comprehended, stats.total, pct)
        } else {
            format!("{}/{} {:.0}%", comprehended, stats.total, pct)
        };
        response.on_hover_text(hover);
    });
}

/// Red (0%) → yellow (50%) → green (100%) gradient, matching the comprehension readout.
fn coverage_color(pct: f32) -> Color32 {
    if pct >= 50.0 {
        let t = (pct - 50.0) / 50.0;
        Color32::from_rgb((180.0 * (1.0 - t)) as u8, 180, 60)
    } else {
        let t = pct / 50.0;
        Color32::from_rgb(180, (180.0 * t) as u8, 60)
    }
}
