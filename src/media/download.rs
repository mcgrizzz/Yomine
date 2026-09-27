//! A pinned ffmpeg build for systems without one. The archives are mirrored as assets of a
//! Yomine release, so a pinned URL keeps serving the file its hash was taken from.

use std::{
    fs::File,
    io::{
        BufReader,
        Read,
    },
    path::{
        Path,
        PathBuf,
    },
};

use liblzma::read::XzDecoder;
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

enum Archive {
    Zip,
    TarXz,
}

struct Build {
    os: &'static str,
    arch: &'static str,
    url: &'static str,
    sha256: &'static str,
    archive: Archive,
}

const BUILDS: &[Build] = &[];

const BINARY: &str = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };

pub fn installed_path() -> PathBuf {
    get_shared_file_path(&format!("ffmpeg/{BINARY}"))
}

pub fn available() -> bool {
    build().is_some()
}

fn build() -> Option<&'static Build> {
    use std::env::consts::{
        ARCH,
        OS,
    };
    BUILDS.iter().find(|b| b.os == OS && b.arch == ARCH)
}

/// Blocking.
pub fn install(progress: Option<&(dyn Fn(String) + Send)>) -> Result<PathBuf, YomineError> {
    let build = build().ok_or_else(|| {
        YomineError::Custom("No ffmpeg download is available for this system".into())
    })?;
    let target = installed_path();
    let dir = target.parent().expect("installed_path has a parent");
    std::fs::create_dir_all(dir)?;
    let archive = dir.join("download.tmp");
    let result = download_with_progress(&http_client()?, build.url, &archive, "ffmpeg", progress)
        .and_then(|()| verify(&archive, build.sha256))
        .and_then(|()| unpack(&archive, &build.archive, &target));
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

/// Writes the archive's `ffmpeg` binary to `target`, wherever it sits in the archive.
fn unpack(archive: &Path, kind: &Archive, target: &Path) -> Result<(), YomineError> {
    let partial = target.with_extension("partial");
    let found = match kind {
        Archive::Zip => {
            let mut zip = zip::ZipArchive::new(BufReader::new(File::open(archive)?))
                .map_err(|e| YomineError::Custom(format!("Bad ffmpeg archive: {e}")))?;
            let name = zip.file_names().find(|n| is_binary(Path::new(n))).map(str::to_string);
            match name {
                Some(name) => {
                    let mut entry = zip
                        .by_name(&name)
                        .map_err(|e| YomineError::Custom(format!("Bad ffmpeg archive: {e}")))?;
                    write(&mut entry, &partial)?;
                    true
                }
                None => false,
            }
        }
        Archive::TarXz => {
            let mut tar = tar::Archive::new(XzDecoder::new(BufReader::new(File::open(archive)?)));
            let mut found = false;
            for entry in tar.entries()? {
                let mut entry = entry?;
                if is_binary(&entry.path()?) {
                    write(&mut entry, &partial)?;
                    found = true;
                    break;
                }
            }
            found
        }
    };
    if !found {
        return Err(YomineError::Custom("The ffmpeg archive has no ffmpeg binary".into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&partial, target)?;
    Ok(())
}

fn is_binary(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == BINARY)
}

fn write(reader: &mut impl Read, path: &Path) -> Result<(), YomineError> {
    std::io::copy(reader, &mut File::create(path)?)?;
    Ok(())
}
