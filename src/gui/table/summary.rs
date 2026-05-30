use eframe::egui::{
    self,
    Color32,
    RichText,
    Sense,
    Ui,
};

use crate::{
    gui::YomineApp,
    tools::knowledge_summary::BandStats,
};

/// Overall knowledge profile, right-aligned and top-anchored so it can sit inline with the
/// file title on the right of the title row.
pub fn ui_knowledge_profile(ui: &mut Ui, app: &YomineApp) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui_knowledge_summary(ui, app);
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

/// Right-aligned section title that groups the readouts beneath it.
fn section_header(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(text).color(ui.visuals().text_color()).size(11.0).strong());
        });
    });
}

/// Compact global JLPT / frequency coverage bars, derived from total Anki knowledge.
fn ui_knowledge_summary(ui: &mut Ui, app: &YomineApp) {
    let Some(summary) = &app.knowledge_summary else {
        return;
    };
    if summary.jlpt.is_empty() && summary.frequency.is_empty() {
        return;
    }

    section_header(ui, "Anki Coverage");

    if !summary.jlpt.is_empty() {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (level, stats) in summary.jlpt.iter().rev() {
                    coverage_mini_bar(ui, level.label(), stats, 40.0, false);
                }
            });
        });
    }

    if !summary.frequency.is_empty() {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (label, stats) in summary.frequency.iter().rev() {
                    coverage_mini_bar(ui, label, stats, 28.0, true);
                }
            });
        });
    }
}

/// One labelled coverage bar: `label` to the left, a filled bar of `bar_width` to the right.
/// `label_in_hover` keeps the label as a prefix in the tooltip (useful for the frequency
/// ranges); the JLPT bars omit it since the level name is already obvious from context.
fn coverage_mini_bar(
    ui: &mut Ui,
    text: &str,
    stats: &BandStats,
    bar_width: f32,
    label_in_hover: bool,
) {
    let frac = stats.comprehension.clamp(0.0, 1.0);
    let color = coverage_color(frac * 100.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;

        // The parent row uses a right-to-left layout, so the bar is allocated first
        // (right side) and the label second (left side) to render as "N5 ████".
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
