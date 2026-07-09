// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The engine reads YOMINE_DATA_DIR in persistence::get_app_data_dir();
    // the flag must become an env var before anything touches the data dir.
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            if let Some(dir) = args.next() {
                std::env::set_var("YOMINE_DATA_DIR", dir);
            }
        }
    }
    yomine_tauri_lib::run()
}
