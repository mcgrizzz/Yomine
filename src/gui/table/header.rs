use eframe::egui::{
    self,
    CornerRadius,
    DragValue,
    Frame,
    Margin,
    RichText,
    Sense,
    TextEdit,
    TextStyle,
    Ui,
};
use egui_double_slider::DoubleSlider;
use egui_extras::TableRow;

use super::{
    state::{
        FrequencyFilter,
        SortDirection,
        SortField,
        SortState,
    },
    YomineApp,
};

pub fn header_cols(_ctx: &egui::Context, mut header: TableRow<'_, '_>, app: &mut YomineApp) {
    header.col(|ui| {
        ui.label(app.theme.heading(ui.ctx(), "Term"));
    });
    header.col(|ui| column_header_sentence(ui, app));
    header.col(|ui| column_header(ui, app, "Frequency", Some(SortField::Frequency)));
    header.col(|ui| column_header(ui, app, "Part of Speech", None));
}

pub fn controls_row(ui: &mut Ui, app: &mut YomineApp) {
    let filter = app.table_state.frequency_filter();
    let mut search = app.table_state.search().to_string();
    let search_width = 280.0;

    Frame::group(ui.style()).inner_margin(Margin::symmetric(8, 1)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;

            ui.allocate_space(egui::vec2(search_width, ui.spacing().interact_size.y));

            ui.separator();

            Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .inner_margin(Margin::symmetric(6, 3))
                .corner_radius(CornerRadius::same(2))
                .show(ui, |ui| {
                    frequency_range_top_controls(ui, filter, app);
                });
        });

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;

            let response = ui.add_sized(
                [search_width, ui.spacing().interact_size.y],
                TextEdit::singleline(&mut search).hint_text("Search terms or sentences..."),
            );
            if response.changed() {
                app.table_state.set_search(search);
            }

            ui.separator();

            Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .inner_margin(Margin::symmetric(6, 3))
                .corner_radius(CornerRadius::same(2))
                .show(ui, |ui| {
                    frequency_range_slider(ui, filter, app);
                });
        });
    });
}

fn column_header(ui: &mut Ui, app: &mut YomineApp, title: &str, sort_field: Option<SortField>) {
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
                toggle_sort_field(app, field, sort_state);
            }

            if let Some(arrow) = sort_arrow_text(ui, sort_state, field, response.hovered()) {
                let arrow_response = ui
                    .add(egui::Label::new(arrow).sense(Sense::click()))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                if arrow_response.clicked() && is_active {
                    let new_direction = sort_state.direction.reversed();
                    app.table_state.set_sort(field, new_direction);
                }
            }
        });
    } else {
        ui.label(app.theme.heading(ui.ctx(), title));
    }
}

fn column_header_sentence(ui: &mut Ui, app: &mut YomineApp) {
    let sort_state = app.table_state.sort_state();

    let is_chronological = matches!(sort_state.field, Some(SortField::Chronological));
    let is_sentence_count = matches!(sort_state.field, Some(SortField::SentenceCount));
    let is_active = is_chronological || is_sentence_count;

    let (current_field, icon, mode_name) = if is_chronological {
        (SortField::Chronological, "🕒", "Chronological")
    } else if is_sentence_count {
        (SortField::SentenceCount, "#", "Sentence Count")
    } else {
        (SortField::Chronological, "🕒", "Chronological")
    };

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
                app.table_state.set_sort(current_field, new_direction);
            } else {
                app.table_state.set_sort(SortField::Chronological, SortDirection::Ascending);
            }
        }

        if let Some(arrow) = sort_arrow_text(ui, sort_state, current_field, response.hovered()) {
            let arrow_response = ui
                .add(egui::Label::new(arrow).sense(Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if arrow_response.clicked() && is_active {
                let new_direction = sort_state.direction.reversed();
                app.table_state.set_sort(current_field, new_direction);
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
                .on_hover_text("Switch between Chronological and Sentence Count");

            if icon_response.clicked() {
                // Cycle through modes while keeping direction
                let next_field = if is_chronological {
                    SortField::SentenceCount
                } else {
                    SortField::Chronological
                };
                app.table_state.set_sort(next_field, sort_state.direction);
            }
        }
    });
}

fn frequency_range_top_controls(ui: &mut Ui, filter: FrequencyFilter, app: &mut YomineApp) {
    if filter.max_bound <= filter.min_bound {
        ui.label("No frequency data available");
        return;
    }

    ui.spacing_mut().item_spacing.x = 8.0;

    let min_bound = filter.min_bound as f64;
    let max_bound = filter.max_bound as f64;
    let mut min_value = filter.selected_min as f64;
    let mut max_value = filter.selected_max as f64;

    ui.label(
        RichText::new("Frequency Range")
            .color(ui.visuals().weak_text_color())
            .text_style(TextStyle::Button),
    );

    ui.label("Min");
    if ui
        .add(
            DragValue::new(&mut min_value)
                .range(min_bound..=max_bound)
                .speed(50.0)
                .fixed_decimals(0),
        )
        .changed()
    {
        min_value = min_value.clamp(min_bound, max_bound);
        if min_value > max_value {
            max_value = min_value;
        }
        app.table_state.set_frequency_range(min_value.round() as u32, max_value.round() as u32);
    }

    ui.label("Max");
    if ui
        .add(
            DragValue::new(&mut max_value)
                .range(min_bound..=max_bound)
                .speed(50.0)
                .fixed_decimals(0),
        )
        .changed()
    {
        max_value = max_value.clamp(min_value, max_bound);
        app.table_state.set_frequency_range(min_value.round() as u32, max_value.round() as u32);
    }

    // Include unknown checkbox with compact label
    let mut include_unknown = filter.include_unknown;
    if ui
        .checkbox(&mut include_unknown, "?")
        .on_hover_text("Include entries without frequency data")
        .changed()
    {
        app.table_state.set_include_unknown(include_unknown);
    }
}

fn frequency_range_slider(ui: &mut Ui, filter: FrequencyFilter, app: &mut YomineApp) {
    if filter.max_bound <= filter.min_bound {
        return;
    }

    let min_bound = filter.min_bound as f64;
    let max_bound = filter.max_bound as f64;
    let mut min_value = filter.selected_min as f64;
    let mut max_value = filter.selected_max as f64;

    let slider_width = 325.0;

    if ui
        .add(
            DoubleSlider::new(&mut min_value, &mut max_value, min_bound..=max_bound)
                .logarithmic(true)
                .width(slider_width),
        )
        .changed()
    {
        let display_min = min_value.round() as u32;
        let display_max = max_value.round() as u32;
        app.table_state.set_frequency_range(display_min, display_max);
    }
}

fn toggle_sort_field(app: &mut YomineApp, field: SortField, current: SortState) {
    if current.field == Some(field) {
        let next_direction = current.direction.reversed();
        app.table_state.set_sort(field, next_direction);
    } else {
        let default_dir = SortState::default_direction(field);
        app.table_state.set_sort(field, default_dir);
    }
}

fn sort_arrow_text(
    ui: &mut Ui,
    sort_state: SortState,
    field: SortField,
    hovered: bool,
) -> Option<RichText> {
    let is_active = matches!(sort_state.field, Some(current) if current == field);
    if !is_active && !hovered {
        return None;
    }

    let direction =
        if is_active { sort_state.direction } else { SortState::default_direction(field) };
    let arrow = match direction {
        SortDirection::Ascending => "↑",
        SortDirection::Descending => "↓",
    };
    let color =
        if is_active { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };

    Some(RichText::new(arrow).color(color).text_style(TextStyle::Small))
}
