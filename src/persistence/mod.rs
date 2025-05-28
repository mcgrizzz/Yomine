use std::{
    fs,
    path::PathBuf,
};

use serde::{
    Deserialize,
    Serialize,
};

const APP_NAME: &str = "yomine";

pub fn get_app_data_dir() -> PathBuf {
    if let Some(data_dir) = dirs::data_local_dir() {
        let app_dir = data_dir.join(APP_NAME);
        let _ = fs::create_dir_all(&app_dir);
        app_dir
    } else {
        PathBuf::from(".")
    }
}

pub fn get_data_file_path(filename: &str) -> PathBuf {
    get_app_data_dir().join(filename)
}

pub fn save_json<T: Serialize>(data: &T, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = get_data_file_path(filename);
    let json = serde_json::to_string_pretty(data)?;
    fs::write(&file_path, json)?;
    println!("Data saved to: {}", file_path.display());
    Ok(())
}

pub fn load_json<T: for<'de> Deserialize<'de> + Default>(
    filename: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let file_path = get_data_file_path(filename);

    if !file_path.exists() {
        return Ok(T::default());
    }

    let json = fs::read_to_string(&file_path)?;
    let data: T = serde_json::from_str(&json)?;
    println!("Data loaded from: {}", file_path.display());
    Ok(data)
}

pub fn load_json_or_default<T: for<'de> Deserialize<'de> + Default>(filename: &str) -> T {
    match load_json::<T>(filename) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to load {}: {}. Using defaults.", filename, e);
            T::default()
        }
    }
}

pub fn delete_data_file(filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = get_data_file_path(filename);
    if file_path.exists() {
        fs::remove_file(&file_path)?;
        println!("Deleted: {}", file_path.display());
    }
    Ok(())
}

pub fn data_file_exists(filename: &str) -> bool {
    get_data_file_path(filename).exists()
}
