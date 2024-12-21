use std::{fs::{self, File}, io::{self, BufReader, BufWriter, Cursor}, path::{Path, PathBuf}};
use reqwest::blocking::get;
use tar::Archive;
use vibrato::{Dictionary, Tokenizer};
use xz2::read::XzDecoder;
use zstd::stream::copy_decode;

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
}

fn cleanup_files(folder_path: &Path, keep_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

fn ensure_dictionary(dict_type: DictType) -> Result<PathBuf, Box<dyn std::error::Error>> {
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



fn load_dictionary(path: &str) -> Result<Dictionary, Box<dyn std::error::Error>> {
    let reader = BufReader::new(File::open(path)?);
    let dict = Dictionary::read(reader)?;
    Ok(dict)
}


pub fn init_vibrato(dict_type: DictType) -> Result<Tokenizer, Box<dyn std::error::Error>> {
    let dict_path = ensure_dictionary(dict_type)?;
    let dict = load_dictionary(dict_path.to_str().unwrap())?;
    let tokenizer = vibrato::Tokenizer::new(dict);
    Ok(tokenizer)
}