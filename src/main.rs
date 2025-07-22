use std::collections::HashMap;

use eframe::egui;
use yomine::gui::YomineApp;

fn main() {
    // Launch GUI immediately without preloading
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_resizable(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Yomine App",
        native_options,
        Box::new(|cc| Ok(Box::new(YomineApp::new(cc, HashMap::new())))),
    );
}
