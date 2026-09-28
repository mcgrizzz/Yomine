//! A pinned ffmpeg build for systems without one, from the release the "Mirror ffmpeg"
//! workflow publishes. That release is never replaced, so each file keeps its hash.

use std::{
    fs::File,
    io::BufReader,
    path::{
        Path,
        PathBuf,
    },
};

use sha2::{
    Digest,
    Sha256,
};

use crate::{
    core::{
        http::{
            download_with_progress,
            http_client,
        },
        YomineError,
    },
    persistence::get_shared_file_path,
};

const MIRROR: &str = "https://github.com/mcgrizzz/Yomine/releases/download/ffmpeg-9.0.2";

/// OS, architecture, file in the mirror, and its SHA-256.
const BUILDS: &[(&str, &str, &str, &str)] = &[
    (
        "windows",
        "x86_64",
        "ffmpeg-windows-x86_64.zip",
        "a872d9a54a196cd63587969b6764dc95a44632743308fc8f0031d5a1f8da77ff",
    ),
    (
        "linux",
        "x86_64",
        "ffmpeg-linux-x86_64.zip",
        "70c46e2eacf6217ebfb8757caa213f326d3babfe6962e09e2e79d1c866fb72e7",
    ),
    (
        "macos",
        "aarch64",
        "ffmpeg-macos-aarch64.zip",
        "12c79949aea108638b41e7f5d686d9478b0af620e6c536f8a98fbc1bce73d615",
    ),
    (
        "macos",
        "x86_64",
        "ffmpeg-macos-x86_64.zip",
        "e13f9582e9bfb061e0dab0b8aafc005b90488abfbdde496afc41b18f9bd3ae8f",
    ),
];

const BINARY: &str = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };

pub fn installed_path() -> PathBuf {
    get_shared_file_path(&format!("ffmpeg/{BINARY}"))
}

pub fn available() -> bool {
    build().is_some()
}

fn build() -> Option<(&'static str, &'static str)> {
    use std::env::consts::{
        ARCH,
        OS,
    };
    BUILDS
        .iter()
        .find(|(os, arch, ..)| *os == OS && *arch == ARCH)
        .map(|(.., file, sha256)| (*file, *sha256))
}

/// Blocking.
pub fn install(progress: Option<&(dyn Fn(String) + Send)>) -> Result<PathBuf, YomineError> {
    let (file, sha256) = build().ok_or_else(|| {
        YomineError::Custom("No ffmpeg download is available for this system".into())
    })?;
    let target = installed_path();
    let dir = target.parent().expect("installed_path has a parent");
    std::fs::create_dir_all(dir)?;
    let archive = dir.join("download.tmp");
    let url = format!("{MIRROR}/{file}");
    let result = download_with_progress(&http_client()?, &url, &archive, "ffmpeg", progress)
        .and_then(|()| verify(&archive, sha256))
        .and_then(|()| unpack(&archive, &target));
    let _ = std::fs::remove_file(&archive);
    result.map(|()| target)
}

fn verify(path: &Path, expected: &str) -> Result<(), YomineError> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut BufReader::new(File::open(path)?), &mut hasher)?;
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        return Err(YomineError::Custom(format!(
            "The ffmpeg download is corrupt (SHA-256 {actual}, expected {expected})"
        )));
    }
    Ok(())
}

fn unpack(archive: &Path, target: &Path) -> Result<(), YomineError> {
    let bad = |e: zip::result::ZipError| YomineError::Custom(format!("Bad ffmpeg archive: {e}"));
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(archive)?)).map_err(bad)?;
    let partial = target.with_extension("partial");
    std::io::copy(&mut zip.by_name(BINARY).map_err(bad)?, &mut File::create(&partial)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&partial, target)?;
    Ok(())
}
