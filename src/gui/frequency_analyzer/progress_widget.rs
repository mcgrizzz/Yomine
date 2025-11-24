use eframe::egui;

use crate::tools::analysis::AnalysisProgress;

const PROGRESS_SPACING: f32 = 5.0;
const LARGE_SPACING: f32 = 10.0;

pub struct AnalysisProgressWidget;

impl AnalysisProgressWidget {
    pub fn show(ui: &mut egui::Ui, ctx: &egui::Context, progress: &mut AnalysisProgress) -> bool {
        let mut cancel_clicked = false;

        let fraction = progress.calculate_fraction();

        ui.add(
            egui::ProgressBar::new(fraction)
                .text(format!("{}/{}", progress.current_file, progress.total_files)),
        );

        ui.add_space(PROGRESS_SPACING);
        ui.label(&progress.message);

        if let Some(start) = progress.start_time {
            let elapsed = start.elapsed().as_secs_f32();

            ui.add_space(PROGRESS_SPACING);
            ui.weak(format!("Elapsed: {:.0}s", elapsed));

            if let Some(smoothed) = progress.calculate_time_estimate(elapsed) {
                ui.weak(format!("Estimated: {:.0}s remaining", smoothed));
            }
        }

        ui.add_space(LARGE_SPACING);
        if ui.button("Cancel Analysis").clicked() {
            cancel_clicked = true;
        }

        ctx.request_repaint();

        cancel_clicked
    }
}
