use std::collections::HashMap;

use eframe::egui::{
    self,
    Color32,
    CornerRadius,
    Id,
    Layout,
    Modal,
    RichText,
    Stroke,
    StrokeKind,
    Ui,
    Vec2,
};

use crate::segmentation::word::POS;

const SCROLL_MAX_HEIGHT: f32 = 520.0;
const CHIP_WIDTH_ESTIMATE: f32 = 160.0;
const COLUMN_GAP: f32 = 8.0;
const VERTICAL_GAP: f32 = 6.0;
const CHIP_ROUNDING: f32 = 18.0;
const HOVER_RING_ROUNDING: f32 = 20.0;
const HOVER_RING_EXPAND: f32 = 2.0;

pub struct PosFiltersModal {
    open: bool,
    raw: HashMap<POS, bool>,
    original: HashMap<POS, bool>,
}

impl Default for PosFiltersModal {
    fn default() -> Self {
        Self::new()
    }
}

impl PosFiltersModal {
    pub fn new() -> Self {
        Self { open: false, raw: default_pos_map(), original: default_pos_map() }
    }

    pub fn open_modal(&mut self, snapshot: HashMap<POS, bool>) {
        let mut current = default_pos_map();

        for (pos, enabled) in snapshot {
            current.insert(pos, enabled);
        }

        self.original = current.clone();
        self.raw = current;
        self.open = true;
    }

    pub const fn is_open(&self) -> bool {
        self.open
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<HashMap<String, bool>> {
        if !self.open {
            return None;
        }

        let mut result = None;
        let modal = Modal::new(Id::new("pos_filters_modal")).show(ctx, |ui| {
            ui.heading("Part of Speech Filters");
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .max_height(SCROLL_MAX_HEIGHT)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.ui_pos_chips(ui);
                });

            ui.separator();

            let dirty = self.is_dirty();
            self.ui_status_bar(ui, dirty);
            ui.add_space(4.0);

            result = self.ui_action_buttons(ui, dirty);
        });

        if modal.should_close() {
            self.open = false;
        }

        result
    }

    fn ui_pos_chips(&mut self, ui: &mut Ui) {
        let all_chips = self.build_chip_list();

        let available_width = ui.available_width();
        let num_columns = ((available_width + COLUMN_GAP) / (CHIP_WIDTH_ESTIMATE + COLUMN_GAP))
            .floor()
            .max(1.0) as usize;
        let items_per_column = all_chips.len().div_ceil(num_columns);

        ui.horizontal(|ui| {
            for col in 0..num_columns {
                let start = col * items_per_column;
                let end = (start + items_per_column).min(all_chips.len());

                if start >= all_chips.len() {
                    break;
                }

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = VERTICAL_GAP;

                    for &(pos, enabled, is_parent) in &all_chips[start..end] {
                        if is_parent {
                            ui_parent_chip_pos(ui, &mut self.raw, pos);
                        } else {
                            ui_chip_pos(ui, &mut self.raw, pos, enabled);
                        }
                    }
                });

                ui.add_space(COLUMN_GAP);
            }
        });
    }

    fn ui_status_bar(&self, ui: &mut Ui, dirty: bool) {
        ui.horizontal(|ui| {
            if dirty {
                ui.colored_label(Color32::YELLOW, "⚠");
                ui.label("Settings have been modified");
            } else {
                ui.colored_label(Color32::TRANSPARENT, "⚠");
                ui.label("");
            }
        });
    }

    fn ui_action_buttons(&mut self, ui: &mut Ui, dirty: bool) -> Option<HashMap<String, bool>> {
        let mut result = None;

        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            let save_clicked = ui.add_enabled(dirty, egui::Button::new("Save Settings")).clicked();
            let cancel_clicked = ui.add_enabled(dirty, egui::Button::new("Cancel")).clicked();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                let reset_clicked = ui.button("Restore Default").clicked();

                if save_clicked {
                    result = Some(
                        self.raw
                            .iter()
                            .map(|(pos, enabled)| (pos.as_key().to_string(), *enabled))
                            .collect(),
                    );
                    self.original = self.raw.clone();
                    ui.close();
                } else if cancel_clicked {
                    self.raw = self.original.clone();
                } else if reset_clicked {
                    self.raw = default_pos_map();
                }
            });
        });

        result
    }

    fn is_dirty(&self) -> bool {
        self.raw != self.original
    }

    fn build_chip_list(&self) -> Vec<(POS, bool, bool)> {
        let mut all_chips = Vec::new();

        all_chips.push((POS::Noun, true, true));
        let noun_on = self.raw.get(&POS::Noun).copied().unwrap_or(true);
        all_chips.extend([
            (POS::ProperNoun, noun_on, false),
            (POS::CompoundNoun, noun_on, false),
            (POS::AdjectivalNoun, noun_on, false),
            (POS::SuruVerb, noun_on, false),
        ]);

        all_chips.push((POS::Number, true, true));
        let number_on = self.raw.get(&POS::Number).copied().unwrap_or(true);
        all_chips.push((POS::Counter, number_on, false));

        all_chips.extend([
            (POS::Verb, true, false),
            (POS::Copula, true, false),
            (POS::Adjective, true, false),
            (POS::Preposition, true, false),
            (POS::Postposition, true, false),
            (POS::Prefix, true, false),
            (POS::Suffix, true, false),
            (POS::Pronoun, true, false),
            (POS::Conjunction, true, false),
            (POS::Interjection, true, false),
            (POS::Adverb, true, false),
            (POS::Determiner, true, false),
            (POS::Symbol, true, false),
            (POS::Expression, true, false),
            (POS::Other, true, false),
            (POS::Unknown, true, false),
        ]);

        all_chips
    }
}

fn ui_parent_chip_pos(ui: &mut Ui, raw: &mut HashMap<POS, bool>, pos: POS) {
    let mut on = raw.get(&pos).copied().unwrap_or(true);
    let resp = ui_chip_button(ui, pos.display_name(), on, true, true);
    if resp.clicked() {
        on = !on;
    }
    raw.insert(pos, on);
}

fn ui_chip_pos(ui: &mut Ui, raw: &mut HashMap<POS, bool>, pos: POS, enabled: bool) {
    let mut on = raw.get(&pos).copied().unwrap_or(true);
    let resp = ui_chip_button(ui, pos.display_name(), on, enabled, false);
    if enabled && resp.clicked() {
        on = !on;
    }
    raw.insert(pos, on);
}

fn ui_chip_button(
    ui: &mut Ui,
    label: &str,
    on: bool,
    enabled: bool,
    strong: bool,
) -> egui::Response {
    let (bg, fg, stroke) = calculate_chip_colors(ui.visuals(), on, enabled);

    let text = if strong {
        RichText::new(label).strong().color(fg)
    } else {
        RichText::new(label).color(fg)
    };

    let btn = egui::Button::new(text)
        .min_size(Vec2::ZERO)
        .stroke(stroke)
        .fill(bg)
        .corner_radius(CornerRadius::same(CHIP_ROUNDING as u8))
        .small();

    let resp = ui.add_enabled(enabled, btn);

    if resp.hovered() && enabled {
        ui_draw_hover_ring(ui, &resp);
    }

    resp
}

fn calculate_chip_colors(
    vis: &egui::Visuals,
    on: bool,
    enabled: bool,
) -> (Color32, Color32, Stroke) {
    if enabled {
        if on {
            (vis.selection.bg_fill, vis.hyperlink_color, vis.selection.stroke)
        } else {
            (vis.widgets.inactive.bg_fill, vis.text_color(), vis.widgets.inactive.bg_stroke)
        }
    } else {
        (
            vis.widgets.inactive.bg_fill.gamma_multiply(0.85),
            vis.weak_text_color(),
            Stroke::new(1.0, vis.weak_text_color()),
        )
    }
}

fn ui_draw_hover_ring(ui: &mut Ui, resp: &egui::Response) {
    let ring_color = ui.visuals().widgets.hovered.fg_stroke.color.linear_multiply(0.6);
    let ring = Stroke::new(1.0, ring_color);
    ui.painter().rect_stroke(
        resp.rect.expand(HOVER_RING_EXPAND),
        CornerRadius::same(HOVER_RING_ROUNDING as u8),
        ring,
        StrokeKind::Outside,
    );
}

fn default_pos_map() -> HashMap<POS, bool> {
    all_pos_variants()
        .iter()
        .copied()
        .map(|p| {
            let default_enabled = !matches!(p, POS::Unknown | POS::Other | POS::Symbol);
            (p, default_enabled)
        })
        .collect()
}

fn all_pos_variants() -> &'static [POS] {
    use POS::*;
    &[
        Noun,
        ProperNoun,
        CompoundNoun,
        NounExpression,
        Pronoun,
        Adjective,
        AdjectivalNoun,
        Adverb,
        Determiner,
        Preposition,
        Postposition,
        Verb,
        SuruVerb,
        Copula,
        Suffix,
        Prefix,
        Conjunction,
        Interjection,
        Number,
        Counter,
        Symbol,
        Expression,
        Other,
        Unknown,
    ]
}
