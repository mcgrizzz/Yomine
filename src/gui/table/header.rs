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

pub fn ui_controls_row(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
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
                        actions.push(UiAction::SetSearch(search.clone()));
                    }
                });

                // Frequency controls and slider stacked vertically, same width
                ui.vertical(|ui| {
                    ui.set_max_width(control_width);

                    // Frequency Range label with controls
                    ui.horizontal(|ui| {
                        ui_frequency_range_controls(ui, filter, app, actions);
                    });

                    // Slider below
                    ui.horizontal(|ui| {
                        ui_frequency_range_slider(ui, filter, app, control_width, actions);
                    });
                });
            });
        });

        // Right side: Comprehension and stats counter (float right)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;

                // Overall comprehension display
                // Only show if we have sentences and successfully fetched Anki data
                if let Some(file_data) = &app.file_data {
                    if !file_data.sentences.is_empty() && !file_data.anki_filtered_terms.is_empty()
                    {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let comp_pct = file_data.file_comprehension * 100.0;

                                    // Color gradient from red (0%) to yellow to green (100%)
                                    let base_color = if comp_pct >= 50.0 {
                                        let t = (comp_pct - 50.0) / 50.0;
                                        egui::Color32::from_rgb((180.0 * (1.0 - t)) as u8, 180, 60)
                                    } else {
                                        let t = comp_pct / 50.0;
                                        egui::Color32::from_rgb(180, (180.0 * t) as u8, 60)
                                    };

                                    // Desaturate by blending with gray
                                    let color = base_color
                                        .blend(egui::Color32::from_gray(140).gamma_multiply(0.6));

                                    ui.label(
                                        RichText::new(format!(
                                            "Estimated comprehension: {:.1}%",
                                            comp_pct
                                        ))
                                        .color(color)
                                        .size(13.0)
                                        .strong(),
                                    )
                                    .on_hover_text(
                                        "Overall estimated comprehension across all sentences",
                                    );
                                },
                            );
                        });
                    }
                }

                // Stats counter
                if let Some(file_data) = &app.file_data {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let visible_count = app.table_state.visible_indices().len();
                            let filtered_count = file_data.terms.len();
                            let total_count = file_data.original_terms.len();
                            let known_count = total_count.saturating_sub(filtered_count);

                            ui.label(
                                RichText::new(format!("{} total", total_count))
                                    .color(ui.visuals().weak_text_color())
                                    .size(12.0),
                            );

                            ui.label(
                                RichText::new("/").color(ui.visuals().weak_text_color()).size(12.0),
                            );

                            if known_count > 0 {
                                // Calculate ignore list and Anki filtered counts
                                let ignore_count = if let Some(ref language_tools) =
                                    app.language_tools
                                {
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

                                let known_label = ui.label(
                                    RichText::new(format!("{} known", known_count))
                                        .color(ui.visuals().weak_text_color())
                                        .size(12.0),
                                );

                                known_label.on_hover_ui(|ui| {
                                    ui.label(format!("Ignore list: {}", ignore_count));
                                    ui.label(format!("Anki filtered: {}", anki_filtered));
                                });

                                ui.label(
                                    RichText::new("/")
                                        .color(ui.visuals().weak_text_color())
                                        .size(12.0),
                                );
                            }

                            ui.label(
                                RichText::new(format!("{} shown", visible_count))
                                    .color(ui.visuals().weak_text_color())
                                    .size(12.0),
                            );
                        });
                    });
                }
            });
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

fn ui_frequency_range_controls(
    ui: &mut Ui,
    filter: FrequencyFilter,
    _app: &YomineApp,
    actions: &mut ActionQueue,
) {
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
        actions.push(UiAction::SetFrequencyRange {
            min: min_value.round() as u32,
            max: max_value.round() as u32,
        });
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
        actions.push(UiAction::SetFrequencyRange {
            min: min_value.round() as u32,
            max: max_value.round() as u32,
        });
    }

    // Include unknown checkbox with compact label
    let mut include_unknown = filter.include_unknown;
    if ui
        .checkbox(&mut include_unknown, "?")
        .on_hover_text("Include entries without frequency data")
        .changed()
    {
        actions.push(UiAction::SetIncludeUnknown(include_unknown));
    }
}

fn ui_frequency_range_slider(
    ui: &mut Ui,
    filter: FrequencyFilter,
    _app: &YomineApp,
    width: f32,
    actions: &mut ActionQueue,
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
        actions.push(UiAction::SetFrequencyRange { min: display_min, max: display_max });
    }
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
