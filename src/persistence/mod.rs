use std::{
    fs,
    io,
    path::{
        Path,
        PathBuf,
    },
};

use serde::{
    Deserialize,
    Serialize,
};

const APP_NAME: &str = "yomine";

pub fn get_app_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("YOMINE_DATA_DIR") {
        let app_dir = PathBuf::from(dir);
        let _ = fs::create_dir_all(&app_dir);
        return app_dir;
    }
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

/// Write via a temp file and rename, so a crash leaves the previous contents intact.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);

    fs::write(&tmp, bytes)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

pub fn save_json<T: Serialize>(data: &T, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = get_data_file_path(filename);
    let json = serde_json::to_string_pretty(data)?;
    atomic_write(&file_path, json.as_bytes())?;
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{
        AtomicU32,
        Ordering,
    };

    use super::*;

    fn scratch(label: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "yomine-atomic-{}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed),
            label
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn replaces_contents_and_leaves_no_temp_file() {
        let dir = scratch("replace");
        let path = dir.join("settings.json");

        atomic_write(&path, b"one").unwrap();
        atomic_write(&path, b"two").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n != "settings.json")
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = scratch("nested");
        let path = dir.join("profiles").join("default").join("settings.json");

        atomic_write(&path, b"{}").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
