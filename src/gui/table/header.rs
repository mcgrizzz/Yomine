use eframe::egui::{
    self,
    RichText,
    Sense,
    TextStyle,
    Ui,
};
use egui_extras::TableRow;

use super::sort::{
    SortDirection,
    SortField,
    SortState,
};
use crate::gui::{
    ActionQueue,
    UiAction,
    YomineApp,
};

pub fn ui_header_cols(
    _ctx: &egui::Context,
    mut header: TableRow<'_, '_>,
    app: &YomineApp,
    actions: &mut ActionQueue,
) {
    header.col(|ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui.label(app.theme.heading(ui.ctx(), "Term"));
    });
    header.col(|ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui_column_header_sentence(ui, app, actions);
    });
    header.col(|ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui_column_header_frequency(ui, app, actions);
    });
    header.col(|ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui.horizontal(|ui| {
            let settings_button = ui
                .add(egui::Button::new("⚙").frame(false).small())
                .on_hover_text("Part of Speech Filters")
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if settings_button.clicked() {
                actions.push(UiAction::OpenPosFilters);
            }

            ui_column_header(ui, app, "POS", None, actions);
        });
    });
}

fn ui_column_header(
    ui: &mut Ui,
    app: &YomineApp,
    title: &str,
    sort_field: Option<SortField>,
    actions: &mut ActionQueue,
) {
    if let Some(field) = sort_field {
        let sort_state = app.table_state.sort_state();
        let is_active = matches!(sort_state.field, Some(current) if current == field);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            let label = app.theme.heading(ui.ctx(), title);
            let mut response = ui
                .add(egui::Label::new(label).sense(Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            response = response.on_hover_ui(|ui| {
                if is_active {
                    let direction_text = match sort_state.direction {
                        SortDirection::Ascending => "ascending",
                        SortDirection::Descending => "descending",
                    };
                    ui.horizontal(|ui| {
                        ui.label("Sorted by");
                        ui.label(RichText::new(title).color(ui.visuals().hyperlink_color).strong());
                        ui.label("in");
                        ui.label(RichText::new(direction_text).color(ui.visuals().warn_fg_color));
                        ui.label("order");
                    });
                } else {
                    ui.label(format!("Sort by {}", title));
                }
            });

            if response.clicked() {
                let direction = if sort_state.field == Some(field) {
                    sort_state.direction.reversed()
                } else {
                    SortState::default_direction(field)
                };
                actions.push(UiAction::SetSort { field, direction });
            }

            if let Some(arrow) = sort_arrow_text(ui, sort_state, field, response.hovered(), app) {
                let arrow_response = ui
                    .add(egui::Label::new(arrow).sense(Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                if arrow_response.clicked() && is_active {
                    let new_direction = sort_state.direction.reversed();
                    actions.push(UiAction::SetSort { field, direction: new_direction });
                }
            }
        });
    } else {
        ui.label(app.theme.heading(ui.ctx(), title));
    }
}

fn draw_column_highlight(ui: &mut Ui, app: &YomineApp) {
    let mut bg_color = app.theme.cyan(ui.ctx());
    bg_color = bg_color.linear_multiply(0.10);

    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 3.0, bg_color);
}

fn ui_column_header_frequency(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
    let sort_state = app.table_state.sort_state();
    let is_active = matches!(sort_state.field, Some(SortField::Frequency));

    if is_active {
        draw_column_highlight(ui, app);
    }

    ui.horizontal(|ui| {
        let settings_button = ui
            .add(egui::Button::new("⚙").frame(false).small())
            .on_hover_text("Frequency Settings")
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        if settings_button.clicked() {
            actions.push(UiAction::OpenFrequencyWeights);
        }

        ui_column_header(ui, app, "Frequency", Some(SortField::Frequency), actions);
    });
}

fn ui_column_header_sentence(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
    let sort_state = app.table_state.sort_state();

    let is_chronological = matches!(sort_state.field, Some(SortField::Chronological));
    let is_sentence_count = matches!(sort_state.field, Some(SortField::SentenceCount));
    let is_comprehension = matches!(sort_state.field, Some(SortField::SentenceComprehension));
    let is_active = is_chronological || is_sentence_count || is_comprehension;

    let (current_field, icon, mode_name) = if is_chronological {
        (SortField::Chronological, "🕒 Chronological", "Chronological")
    } else if is_sentence_count {
        (SortField::SentenceCount, "# Sentence Count", "Sentence Count")
    } else if is_comprehension {
        (SortField::SentenceComprehension, "📊 Estimated Comprehension", "Comprehension")
    } else {
        (SortField::Chronological, "🕒 Chronological", "Chronological")
    };

    if is_active {
        draw_column_highlight(ui, app);
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        let label = app.theme.heading(ui.ctx(), "Sentence");
        let mut response = ui
            .add(egui::Label::new(label).sense(Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        response = response.on_hover_ui(|ui| {
            if is_active {
                let direction_text = match sort_state.direction {
                    SortDirection::Ascending => "ascending",
                    SortDirection::Descending => "descending",
                };
                ui.horizontal(|ui| {
                    ui.label("Sorted by");
                    ui.label(RichText::new(mode_name).color(ui.visuals().hyperlink_color).strong());
                    ui.label("in");
                    ui.label(RichText::new(direction_text).color(ui.visuals().warn_fg_color));
                    ui.label("order");
                });
            } else {
                ui.label("Sort by Sentence");
            }
        });

        if response.clicked() {
            if is_active {
                let new_direction = sort_state.direction.reversed();
                actions.push(UiAction::SetSort { field: current_field, direction: new_direction });
            } else {
                actions.push(UiAction::SetSort {
                    field: SortField::Chronological,
                    direction: SortDirection::Ascending,
                });
            }
        }

        if let Some(arrow) = sort_arrow_text(ui, sort_state, current_field, response.hovered(), app)
        {
            let arrow_response = ui
                .add(egui::Label::new(arrow).sense(Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if arrow_response.clicked() && is_active {
                let new_direction = sort_state.direction.reversed();
                actions.push(UiAction::SetSort { field: current_field, direction: new_direction });
            }
        }

        if is_active {
            let icon_response = ui
                .add(
                    egui::Label::new(
                        RichText::new(icon)
                            .color(ui.visuals().weak_text_color())
                            .text_style(TextStyle::Small),
                    )
                    .sense(Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Cycle between Chronological, Sentence Count, and Comprehension");

            if icon_response.clicked() {
                // Cycle through three modes while keeping direction
                let next_field = if is_chronological {
                    SortField::SentenceCount
                } else if is_sentence_count {
                    SortField::SentenceComprehension
                } else {
                    SortField::Chronological
                };
                actions
                    .push(UiAction::SetSort { field: next_field, direction: sort_state.direction });
            }
        }
    });
}

fn sort_arrow_text(
    ui: &mut Ui,
    sort_state: SortState,
    field: SortField,
    hovered: bool,
    app: &YomineApp,
) -> Option<RichText> {
    let is_active = matches!(sort_state.field, Some(current) if current == field);
    if !is_active && !hovered {
        return None;
    }

    let direction =
        if is_active { sort_state.direction } else { SortState::default_direction(field) };
    let arrow = match direction {
        SortDirection::Ascending => "⬆",
        SortDirection::Descending => "⬇",
    };
    let color = if is_active { app.theme.cyan(ui.ctx()) } else { ui.visuals().weak_text_color() };

    Some(RichText::new(arrow).color(color).text_style(TextStyle::Small))
}
