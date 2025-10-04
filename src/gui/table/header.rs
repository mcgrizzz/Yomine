use eframe::egui::{
    self,
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
    filter::FrequencyFilter,
    sort::{
        SortDirection,
        SortField,
        SortState,
    },
    YomineApp,
};

pub fn ui_header_cols(_ctx: &egui::Context, mut header: TableRow<'_, '_>, app: &mut YomineApp) {
    header.col(|ui| {
        ui.label(app.theme.heading(ui.ctx(), "Term"));
    });
    header.col(|ui| ui_column_header_sentence(ui, app));
    header.col(|ui| {
        ui.horizontal(|ui| {
            let settings_button = ui
                .add(egui::Button::new("⚙").frame(false).small())
                .on_hover_text("Frequency Settings")
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if settings_button.clicked() {
                let frequency_manager =
                    app.language_tools.as_ref().map(|tools| tools.frequency_manager.as_ref());
                app.frequency_weights_modal.open_modal(&app.settings_data, frequency_manager);
            }

            ui_column_header(ui, app, "Frequency", Some(SortField::Frequency));
        });
    });
    header.col(|ui| {
        ui.horizontal(|ui| {
            let settings_button = ui
                .add(egui::Button::new("⚙").frame(false).small())
                .on_hover_text("Part of Speech Filters")
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if settings_button.clicked() {
                app.pos_filters_modal.open_modal(app.table_state.pos_snapshot());
            }

            ui_column_header(ui, app, "POS", None);
        });
    });
}

pub fn ui_controls_row(ui: &mut Ui, app: &mut YomineApp) {
    let filter = app.table_state.frequency_filter();
    let mut search = app.table_state.search().to_string();
    let control_width = 350.0;

    ui.horizontal(|ui| {
        // Left side: Search bar and frequency controls (float left)
        ui.vertical(|ui| {
            Frame::group(ui.style()).inner_margin(Margin::symmetric(8, 1)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;

                // Search bar
                ui.horizontal(|ui| {
                    let response = ui.add_sized(
                        [control_width, 22.0],
                        TextEdit::singleline(&mut search).hint_text("Search terms or sentences..."),
                    );
                    if response.changed() {
                        app.table_state.set_search(search);
                    }
                });

                // Frequency controls and slider stacked vertically, same width
                ui.vertical(|ui| {
                    ui.set_max_width(control_width);

                    // Frequency Range label with controls
                    ui.horizontal(|ui| {
                        ui_frequency_range_controls(ui, filter, app);
                    });

                    // Slider below
                    ui.horizontal(|ui| {
                        ui_frequency_range_slider(ui, filter, app, control_width);
                    });
                });
            });
        });

        // Right side: Stats counter (float right)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let visible_count = app.table_state.visible_indices().len();
            let filtered_count = app.terms.len();
            let total_count = app.original_terms.len();
            let known_count = total_count.saturating_sub(filtered_count);

            ui.label(
                RichText::new(format!("{} total", total_count))
                    .color(ui.visuals().weak_text_color())
                    .text_style(TextStyle::Small),
            );

            ui.label(
                RichText::new("/")
                    .color(ui.visuals().weak_text_color())
                    .text_style(TextStyle::Small),
            );

            if known_count > 0 {
                // Calculate ignore list and Anki filtered counts
                let ignore_count = if let Some(ref language_tools) = app.language_tools {
                    if let Ok(ignore_list) = language_tools.ignore_list.lock() {
                        app.original_terms
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

                let known_label = ui.label(
                    RichText::new(format!("{} known", known_count))
                        .color(ui.visuals().weak_text_color())
                        .text_style(TextStyle::Small),
                );

                known_label.on_hover_ui(|ui| {
                    ui.label(format!("Ignore list: {}", ignore_count));
                    ui.label(format!("Anki filtered: {}", anki_filtered));
                });

                ui.label(
                    RichText::new("/")
                        .color(ui.visuals().weak_text_color())
                        .text_style(TextStyle::Small),
                );
            }

            ui.label(
                RichText::new(format!("{} shown", visible_count))
                    .color(ui.visuals().weak_text_color())
                    .text_style(TextStyle::Small),
            );
        });
    });
}

fn ui_column_header(ui: &mut Ui, app: &mut YomineApp, title: &str, sort_field: Option<SortField>) {
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

fn ui_column_header_sentence(ui: &mut Ui, app: &mut YomineApp) {
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

fn ui_frequency_range_controls(ui: &mut Ui, filter: FrequencyFilter, app: &mut YomineApp) {
    if filter.max_bound <= filter.min_bound {
        ui.label(RichText::new("No frequency data available").text_style(TextStyle::Small));
        return;
    }

    ui.spacing_mut().item_spacing.x = 6.0;

    let min_bound = filter.min_bound as f64;
    let max_bound = filter.max_bound as f64;
    let mut min_value = filter.selected_min as f64;
    let mut max_value = filter.selected_max as f64;

    ui.label(
        RichText::new("Frequency Range")
            .color(ui.visuals().weak_text_color())
            .text_style(TextStyle::Small),
    );

    ui.label(RichText::new("Min").text_style(TextStyle::Small));
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

    ui.label(RichText::new("Max").text_style(TextStyle::Small));
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

fn ui_frequency_range_slider(
    ui: &mut Ui,
    filter: FrequencyFilter,
    app: &mut YomineApp,
    width: f32,
) {
    if filter.max_bound <= filter.min_bound {
        return;
    }

    let min_bound = filter.min_bound as f64;
    let max_bound = filter.max_bound as f64;
    let mut min_value = filter.selected_min as f64;
    let mut max_value = filter.selected_max as f64;

    if ui
        .add(
            DoubleSlider::new(&mut min_value, &mut max_value, min_bound..=max_bound)
                .logarithmic(true)
                .width(width),
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
        SortDirection::Ascending => "⇧",
        SortDirection::Descending => "⇩",
    };
    let color =
        if is_active { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };

    Some(RichText::new(arrow).color(color).text_style(TextStyle::Small))
}
