use eframe::egui::{
    Color32,
    DragValue,
    RichText,
    TextEdit,
    TextStyle,
    Ui,
};
use egui_double_slider::DoubleSlider;

use super::filter::FrequencyFilter;
use crate::gui::{
    ActionQueue,
    UiAction,
    YomineApp,
};

pub fn ui_controls_row(ui: &mut Ui, app: &YomineApp, actions: &mut ActionQueue) {
    let filter = app.table_state.frequency_filter();
    let mut search = app.table_state.search().to_string();
    let control_width = 350.0;

    // Left-aligned with the title and current-file summary (no surrounding frame indent).
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;

        // Search bar
        let response = ui.add_sized(
            [control_width, 22.0],
            TextEdit::singleline(&mut search).hint_text("Search terms or sentences..."),
        );
        if response.changed() {
            actions.push(UiAction::SetSearch(search.clone()));
        }

        // Frequency controls and slider stacked vertically, same width.
        ui.vertical(|ui| {
            ui.set_max_width(control_width);

            ui.horizontal(|ui| {
                ui_frequency_range_controls(ui, filter, app, actions);
            });

            ui.horizontal(|ui| {
                ui_frequency_range_slider(ui, filter, app, control_width, actions);
            });
        });
    });
}

fn ui_frequency_range_controls(
    ui: &mut Ui,
    filter: FrequencyFilter,
    _app: &YomineApp,
    actions: &mut ActionQueue,
) {
    if filter.max_bound <= filter.min_bound {
        ui.label(
            RichText::new("No frequency data available")
                .text_style(TextStyle::Heading)
                .color(Color32::RED),
        );
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
