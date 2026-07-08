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
