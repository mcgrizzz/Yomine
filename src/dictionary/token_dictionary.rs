use std::{fs::{self, File}, io::{self, BufReader, BufWriter, Cursor}, path::{Path, PathBuf}};

use reqwest::blocking::get;
use tar::Archive;
use vibrato::Dictionary;
use xz2::bufread::XzDecoder;
use zstd::stream::copy_decode;

use crate::core::YomineError;



const DICT_DIR: &str = "dictionaries";

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
            DictType::Unidic => {
                (10, 11)
            },
            DictType::Ipadic => {
                (6, 8) //8 is the surface form reading.. sometimes? Let's use unidic for now
            }
        }
    }
}

fn cleanup_files(folder_path: &Path, keep_file: &Path) -> Result<(), YomineError> {
    println!("Cleaning up intermediate files...");

    // Iterate through all files and directories in the folder
    for entry in fs::read_dir(folder_path)? {
        let entry = entry?;
        let path = entry.path();

        // Skip the file we want to keep
        if path == keep_file {
            continue;
        }

        // Remove directories or files
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }

    println!("Cleanup complete. Retained file: {:?}", keep_file);
    Ok(())
}

pub fn ensure_dictionary(dict_type: &DictType) -> Result<PathBuf, YomineError> {
    let url = dict_type.url();
    let folder_name = dict_type.folder_name();
    let extract_path = Path::new(DICT_DIR).join(folder_name);
    let final_dic_path = extract_path.join("system.dic");

    if final_dic_path.exists() {
        println!("Dictionary already exists: {:?}", final_dic_path);
        return Ok(final_dic_path);
    }

    fs::create_dir_all(DICT_DIR)?;

    let download_path = Path::new(DICT_DIR).join(format!("{}.tar.gz", folder_name));
    if !download_path.exists() {
        println!("Downloading dictionary from {}...", url);
        let response = get(url)?;
        let mut file = File::create(&download_path)?;
        io::copy(&mut Cursor::new(response.bytes()?), &mut file)?;
        println!("Downloaded dictionary to {:?}", download_path);
    }

    let tar_path = download_path.with_extension("tar");
    if !tar_path.exists() {
        println!("Decompressing XZ to TAR...");
        let tar_xz_file = File::open(&download_path)?;
        let mut tar_file = File::create(&tar_path)?;
        let mut xz_decoder = XzDecoder::new(BufReader::new(tar_xz_file));
        io::copy(&mut xz_decoder, &mut tar_file)?;
        println!("Decompressed TAR file to {:?}", tar_path);
    }

    if !extract_path.exists() {
        println!("Extracting TAR archive...");
        let tar_file = File::open(&tar_path)?;
        let mut archive = Archive::new(BufReader::new(tar_file));
        archive.unpack(&extract_path)?;
        println!("Extracted TAR archive to {:?}", extract_path);
    }

    let zst_path = extract_path.join(folder_name).join("system.dic.zst");
    if !final_dic_path.exists() {
        println!("Decompressing Zstandard to .dic...");
        let zst_file = File::open(&zst_path)?;
        let dic_file = File::create(&final_dic_path)?;
        copy_decode(BufReader::new(zst_file), BufWriter::new(dic_file))?;
        println!("Decompressed .dic file to {:?}", final_dic_path);
    }
    
    //Clean up extra files
    cleanup_files(&extract_path, &final_dic_path)?;
    fs::remove_file(&download_path)?;
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