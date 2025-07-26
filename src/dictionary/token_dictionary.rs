use std::{
    fs::{
        self,
        File,
    },
    io::{
        self,
        BufReader,
        BufWriter,
        Cursor,
    },
    path::{
        Path,
        PathBuf,
    },
};

use liblzma::read::XzDecoder;
use reqwest::blocking::get;
use tar::Archive;
use vibrato::Dictionary;
use zstd::stream::copy_decode;

use crate::{
    core::YomineError,
    persistence::get_app_data_dir,
};

fn get_tokenizer_dict_dir() -> std::path::PathBuf {
    get_app_data_dir().join("dictionaries").join("tokenizer")
}

pub enum DictType {
    Unidic,
    Ipadic,
}

impl DictType {
    fn url(&self) -> &str {
        match self {
            DictType::Unidic => {
                "https://github.com/daac-tools/vibrato/releases/download/v0.5.0/bccwj-suw+unidic-cwj-3_1_1.tar.xz"
            }
            DictType::Ipadic => {
                "https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz"
            }
        }
    }

    fn folder_name(&self) -> &str {
        match self {
            DictType::Unidic => "bccwj-suw+unidic-cwj-3_1_1",
            DictType::Ipadic => "ipadic-mecab-2_7_0",
        }
    }

    // lemma_form index, lemma_reading index
    pub fn lemma_indices(&self) -> (usize, usize) {
        match self {
            DictType::Unidic => (10, 11),
            DictType::Ipadic => {
                (6, 8) //8 is the surface form reading.. sometimes? Let's use unidic for now
            }
        }
    }
}

fn cleanup_files(folder_path: &Path, keep_files: &[&str]) -> Result<(), YomineError> {
    let keep_paths: Vec<PathBuf> = keep_files.iter().map(|f| folder_path.join(f)).collect();
    println!("Cleaning up intermediate files...");

    // Iterate through all files and directories in the folder
    for entry in fs::read_dir(folder_path).map_err(|e| {
        YomineError::Custom(format!("Failed to read directory during cleanup: {}", e))
    })? {
        let entry = entry.map_err(|e| {
            YomineError::Custom(format!("Failed to get directory entry during cleanup: {}", e))
        })?;
        let path = entry.path();

        // Skip the files we want to keep
        if keep_paths.contains(&path) {
            continue;
        }

        // Remove directories or files
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|e| {
                YomineError::Custom(format!(
                    "Failed to remove directory during cleanup: {:?} - {}",
                    path, e
                ))
            })?;
        } else {
            fs::remove_file(&path).map_err(|e| {
                YomineError::Custom(format!(
                    "Failed to remove file during cleanup: {:?} - {}",
                    path, e
                ))
            })?;
        }
    }

    println!("Cleanup complete. Retained files: {:?}", keep_files);
    Ok(())
}

pub fn ensure_dictionary(dict_type: &DictType) -> Result<PathBuf, YomineError> {
    let url = dict_type.url();
    let folder_name = dict_type.folder_name();
    let dict_dir = get_tokenizer_dict_dir();
    let extract_path = dict_dir.join(folder_name);
    let final_dic_path = extract_path.join("system.dic");

    if final_dic_path.exists() {
        println!("Dictionary already exists: {:?}", final_dic_path);
        return Ok(final_dic_path);
    }

    fs::create_dir_all(&dict_dir).map_err(|e| {
        YomineError::Custom(format!("Failed to create dictionary directory {:?}: {}", dict_dir, e))
    })?;

    // Clean up any partial downloads or extractions from previous attempts
    let download_path = dict_dir.join(format!("{}.tar.xz", folder_name));
    let tar_path = dict_dir.join(format!("{}.tar", folder_name));
    fs::remove_file(&download_path).ok();
    fs::remove_file(&tar_path).ok();
    fs::remove_dir_all(&extract_path).ok();

    println!("Downloading dictionary from {}...", url);
    let response = get(url).map_err(|e| {
        YomineError::Custom(format!("Failed to download dictionary from {}: {}", url, e))
    })?;

    let mut file = File::create(&download_path).map_err(|e| {
        YomineError::Custom(format!("Failed to create download file {:?}: {}", download_path, e))
    })?;

    io::copy(&mut Cursor::new(response.bytes()?), &mut file).map_err(|e| {
        YomineError::Custom(format!(
            "Failed to write downloaded data to {:?}: {}",
            download_path, e
        ))
    })?;

    println!("Downloaded dictionary to {:?}", download_path);

    let metadata = download_path.metadata().map_err(|e| {
        YomineError::Custom(format!(
            "Failed to get metadata for downloaded file {:?}: {}",
            download_path, e
        ))
    })?;

    if metadata.len() == 0 {
        return Err(YomineError::Custom(format!(
            "Downloaded file {:?} is empty. Check your internet connection.",
            download_path
        )));
    }

    println!("Decompressing XZ to TAR...");
    let tar_xz_file = File::open(&download_path).map_err(|e| {
        YomineError::Custom(format!("Failed to open downloaded file {:?}: {}", download_path, e))
    })?;

    let mut tar_file = File::create(&tar_path).map_err(|e| {
        YomineError::Custom(format!("Failed to create TAR file {:?}: {}", tar_path, e))
    })?;

    let mut xz_decoder = XzDecoder::new(BufReader::new(tar_xz_file));
    io::copy(&mut xz_decoder, &mut tar_file).map_err(|e| {
        YomineError::Custom(format!(
            "Failed to decompress XZ to TAR: {}. Possible corrupt download.",
            e
        ))
    })?;

    println!("Decompressed XZ file to {:?}", tar_path);

    println!("Extracting TAR archive...");
    let tar_file = File::open(&tar_path).map_err(|e| {
        YomineError::Custom(format!("Failed to open TAR file {:?}: {}", tar_path, e))
    })?;
    let mut archive = Archive::new(BufReader::new(tar_file));
    archive.unpack(&extract_path).map_err(|e| {
        YomineError::Custom(format!("Failed to unpack TAR to {:?}: {}.", dict_dir, e))
    })?;
    println!("Extracted TAR archive to {:?}", extract_path);

    let zst_path = extract_path.join(folder_name).join("system.dic.zst");
    if !zst_path.exists() {
        return Err(YomineError::Custom(format!(
            "ZST file not found at {:?} after extraction.",
            zst_path
        )));
    }

    println!("Decompressing Zstandard to .dic...");
    let zst_file = File::open(&zst_path).map_err(|e| {
        YomineError::Custom(format!("Failed to open ZST file {:?}: {}.", zst_path, e))
    })?;
    let dic_file = File::create(&final_dic_path).map_err(|e| {
        YomineError::Custom(format!("Failed to create .dic file {:?}: {}", final_dic_path, e))
    })?;
    copy_decode(BufReader::new(zst_file), BufWriter::new(dic_file)).map_err(|e| {
        YomineError::Custom(format!("Failed to decompress ZST to {:?}: {}.", final_dic_path, e))
    })?;
    println!("Decompressed ZST file to {:?}", final_dic_path);

    let inner_path = extract_path.join(folder_name);
    fs::rename(inner_path.join("BSD"), extract_path.join("BSD"))
        .map_err(|e| YomineError::Custom(format!("Failed to move BSD file: {}", e)))?;
    fs::rename(inner_path.join("NOTICE"), extract_path.join("NOTICE"))
        .map_err(|e| YomineError::Custom(format!("Failed to move NOTICE file: {}", e)))?;

    //Clean up extra files
    let keep_files = ["system.dic", "BSD", "NOTICE"];
    cleanup_files(&extract_path, &keep_files)?;
    println!("Removing download {:?}", &download_path);
    fs::remove_file(&download_path)?;
    println!("Removing tar {:?}", &tar_path);
    fs::remove_file(&tar_path)?;

    Ok(final_dic_path)
}

pub fn load_dictionary(path: &str) -> Result<Dictionary, YomineError> {
    let reader = BufReader::new(File::open(path)?);
    let dict = Dictionary::read(reader)?;
    Ok(dict)
}

pub fn is_all_kana(word: &str) -> bool {
    word.chars().all(|c| {
        (c >= '\u{3040}' && c <= '\u{309F}') || //Hiragana
            (c >= '\u{30A0}' && c <= '\u{30FF}') //Katakana
    })
}
