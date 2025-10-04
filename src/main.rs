use std::collections::HashMap;

use eframe::egui;
use yomine::gui::YomineApp;

fn main() {
    // Launch GUI immediately without preloading

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("Missing Icon File");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            //.with_inner_size([825.0, 475.0])
            .with_min_inner_size([1400.0, 805.0])
            .with_resizable(true)
            .with_icon(icon), // Temporarily disable persistence to reset window size
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Yomine App",
        native_options,
        Box::new(|cc| Ok(Box::new(YomineApp::new(cc, HashMap::new())))),
    );
}
