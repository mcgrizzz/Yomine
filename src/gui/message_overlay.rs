use eframe::egui;

use crate::gui::theme::Theme;

pub struct MessageOverlay {
    pub active: bool,
    pub message: Option<String>,
}

impl MessageOverlay {
    pub fn new() -> Self {
        Self { active: true, message: Some("Loading language tools...".to_string()) }
    }

    pub fn set_message(&mut self, message: String) {
        self.message = Some(message);
        self.active = true;
    }

    pub fn clear_message(&mut self) {
        self.message = None;
        self.active = false;
    }

    pub fn show(&self, ctx: &egui::Context, theme: &Theme) {
        if self.active {
            // Background overlay
            egui::Area::new(egui::Id::new("message_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::Pos2::new(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen_size = ui.ctx().screen_rect().size();
                    ui.allocate_space(screen_size);
                    ui.painter().rect_filled(
                        ui.ctx().screen_rect(),
                        0.0,
                        egui::Color32::from_black_alpha(120),
                    );
                });

            let message: String = match &self.message {
                None => "Loading...".to_string(),
                Some(value) => value.to_string(),
            };

            // Message box
            egui::Window::new("message_box")
                .order(egui::Order::Foreground)
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .fixed_size(egui::Vec2::new(200.0, 100.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::new(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.style_mut().visuals.window_fill =
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 150);
                    ui.style_mut().visuals.window_stroke = egui::Stroke::new(2.0, theme.red(ui.ctx()));

                    ui.centered_and_justified(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(message);
                    });
                });
        }
    }
}

impl Default for MessageOverlay {
    fn default() -> Self {
        Self::new()
    }
}
