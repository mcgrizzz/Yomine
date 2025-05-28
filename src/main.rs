use std::collections::HashMap;

use yomine::{
    anki::FieldMapping,
    gui::YomineApp,
};

fn main() {
    let mut model_mapping: HashMap<String, FieldMapping> = HashMap::new();
    model_mapping.insert(
        "Lapis".to_string(),
        FieldMapping {
            term_field: "Expression".to_string(),
            reading_field: "ExpressionReading".to_string(),
        },
    );

    model_mapping.insert(
        "Kaishi 1.5k".to_string(),
        FieldMapping { term_field: "Word".to_string(), reading_field: "Word Reading".to_string() },
    );

    // Launch GUI immediately without preloading
    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Yomine App",
        native_options,
        Box::new(|cc| Ok(Box::new(YomineApp::new(cc, model_mapping)))),
    );
}
