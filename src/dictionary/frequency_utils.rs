use std::fs;

use crate::{
    core::errors::YomineError,
    dictionary::frequency_manager::get_frequency_dict_dir,
};

pub fn copy_frequency_dictionaries(
    zip_paths: Vec<std::path::PathBuf>,
) -> Result<usize, YomineError> {
    let frequency_dir = get_frequency_dict_dir();

    fs::create_dir_all(&frequency_dir)?;

    let mut copied_count = 0;

    for zip_path in zip_paths {
        if let Some(filename) = zip_path.file_name() {
            let destination = frequency_dir.join(filename);

            // Skip if file already exists in destination
            if destination.exists() {
                println!(
                    "Skipping '{}' - already exists in frequency dictionary folder",
                    filename.to_string_lossy()
                );
                continue;
            }

            // Copy the zip file to the frequency dictionary directory
            fs::copy(&zip_path, &destination)?;
            copied_count += 1;

            println!(
                "Copied frequency dictionary: {} -> {}",
                zip_path.display(),
                destination.display()
            );
        }
    }

    Ok(copied_count)
}

pub fn select_frequency_dictionary_zips() -> Option<Vec<std::path::PathBuf>> {
    rfd::FileDialog::new()
        .add_filter("Yomitan Frequency Dictionaries", &["zip"])
        .add_filter("All Files", &["*"])
        .pick_files()
}

pub fn handle_frequency_dictionary_copy() -> Result<usize, YomineError> {
    if let Some(zip_paths) = select_frequency_dictionary_zips() {
        if zip_paths.is_empty() {
            return Ok(0);
        }

        let copied_count = copy_frequency_dictionaries(zip_paths)?;

        if copied_count > 0 {
            println!("Successfully added {} frequency dictionaries. Restart the application to load them.", copied_count);
        } else {
            println!("No new frequency dictionaries were added.");
        }

        Ok(copied_count)
    } else {
        Ok(0)
    }
}
