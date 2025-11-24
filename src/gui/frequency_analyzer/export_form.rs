use eframe::egui;

use crate::tools::analysis::ExportOptions;

const GRID_HORIZONTAL_SPACING: f32 = 10.0;
const GRID_VERTICAL_SPACING: f32 = 4.0;
const SMALL_SPACING: f32 = 4.0;
const MEDIUM_SPACING: f32 = 6.0;

pub struct ExportFormWidget;

impl ExportFormWidget {
    pub fn show(ui: &mut egui::Ui, export_options: &mut ExportOptions) -> bool {
        let mut export_clicked = false;

        ui.label("Export Options:");
        ui.add_space(SMALL_SPACING);

        egui::Grid::new("export_form_grid")
            .num_columns(2)
            .spacing([GRID_HORIZONTAL_SPACING, GRID_VERTICAL_SPACING])
            .show(ui, |ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut export_options.dict_name);
                ui.end_row();

                ui.label("Author:");
                ui.text_edit_singleline(&mut export_options.dict_author);
                ui.end_row();

                ui.label("URL:");
                ui.text_edit_singleline(&mut export_options.dict_url);
                ui.end_row();

                ui.label("Revision prefix:");
                let response = ui.text_edit_singleline(&mut export_options.revision_prefix);
                response.on_hover_text(
                    "Optional prefix for the revision field (e.g., 'myproject'). \
                     The final revision will be '<prefix>.frequency.<date>'.",
                );
                ui.end_row();

                ui.label("Description:");
                ui.text_edit_multiline(&mut export_options.dict_description);
                ui.end_row();
            });

        ui.add_space(MEDIUM_SPACING);

        ui.horizontal(|ui| {
            ui.checkbox(&mut export_options.export_yomitan, "Export as Yomitan ZIP");
            ui.checkbox(&mut export_options.export_csv, "Export as CSV");
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut export_options.pretty_json, "Pretty JSON output");
            ui.checkbox(&mut export_options.exclude_hapax, "Exclude hapax (occurrences=1)");
        });

        ui.add_space(MEDIUM_SPACING);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let can_export = export_options.export_yomitan || export_options.export_csv;
            if ui.add_enabled(can_export, egui::Button::new("Export…")).clicked() {
                export_clicked = true;
            }
        });

        export_clicked
    }
}
