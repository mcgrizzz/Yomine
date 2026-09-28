//! Locating and running ffmpeg.

use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{
        Path,
        PathBuf,
    },
    process::{
        Command,
        Output,
        Stdio,
    },
    sync::OnceLock,
};

use super::{
    clip::{
        self,
        MediaFormat,
    },
    download,
    probe::{
        self,
        MediaInfo,
    },
};
use crate::core::YomineError;

pub struct Ffmpeg {
    path: PathBuf,
    encoders: OnceLock<HashSet<String>>,
}

/// A file in the temp folder, removed when dropped.
pub struct Encoded {
    pub path: PathBuf,
    pub extension: &'static str,
}

impl Drop for Encoded {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn find(configured: Option<&Path>) -> Option<Ffmpeg> {
    let mut candidates: Vec<PathBuf> = configured.into_iter().map(Path::to_path_buf).collect();
    candidates.push("ffmpeg".into());
    // Apps started from Finder don't get the shell's PATH.
    if cfg!(target_os = "macos") {
        candidates.extend(["/opt/homebrew/bin/ffmpeg".into(), "/usr/local/bin/ffmpeg".into()]);
    }
    candidates.push(download::installed_path());
    candidates.into_iter().map(Ffmpeg::new).find(|f| f.run(["-version"]).is_ok())
}

impl Ffmpeg {
    pub fn new(path: PathBuf) -> Self {
        Self { path, encoders: OnceLock::new() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.path);
        command.stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }

    pub(super) fn run<I, S>(&self, args: I) -> Result<Output, YomineError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.command().args(args).output()?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(YomineError::Custom(format!("ffmpeg failed: {}", last_line(&output.stderr))))
        }
    }

    pub fn probe(&self, input: &Path) -> Result<MediaInfo, YomineError> {
        // Without an output ffmpeg exits with an error after listing the input.
        let output = self.command().arg("-hide_banner").arg("-i").arg(input).output()?;
        let info = probe::parse(&String::from_utf8_lossy(&output.stderr));
        if info.streams.is_empty() {
            return Err(YomineError::Custom(format!(
                "ffmpeg couldn't read {}: {}",
                input.display(),
                last_line(&output.stderr)
            )));
        }
        Ok(info)
    }

    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders
            .get_or_init(|| {
                let Ok(output) = self.run(["-hide_banner", "-encoders"]) else {
                    return HashSet::new();
                };
                // Rows look like " A....D libmp3lame  libmp3lame MP3 (MPEG audio layer 3)".
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| {
                        let mut words = line.split_whitespace();
                        let flags = words.next()?;
                        let name = words.next().filter(|name| *name != "=")?;
                        (flags.len() == 6).then_some(name)
                    })
                    .map(str::to_string)
                    .collect()
            })
            .contains(name)
    }

    pub fn encode_audio(
        &self,
        input: &Path,
        info: &MediaInfo,
        cue: (f64, f64),
        format: &MediaFormat,
    ) -> Result<Encoded, YomineError> {
        let stream = info
            .audio_stream()
            .ok_or_else(|| YomineError::Custom("The video has no audio".into()))?;
        let (audio, encoder) = clip::pick_audio_encoder(format.audio, |e| self.has_encoder(e))
            .ok_or_else(|| {
                YomineError::Custom("This ffmpeg build can't encode MP3 or Opus audio".into())
            })?;
        let out = temp_output(audio.extension());
        let range = clip::audio_range(cue.0, cue.1, format, info.duration);
        self.run(clip::audio_args(input, stream.index, range, encoder, &out.path))?;
        Ok(out)
    }

    pub fn encode_frame(
        &self,
        input: &Path,
        info: &MediaInfo,
        cue: (f64, f64),
        format: &MediaFormat,
    ) -> Result<Encoded, YomineError> {
        let stream = info
            .video_stream()
            .ok_or_else(|| YomineError::Custom("The file has no video".into()))?;
        let out = temp_output(format.image.extension());
        let time = clip::frame_time(cue.0, cue.1, info.duration);
        self.run(clip::frame_args(input, stream.index, time, format, &out.path))?;
        Ok(out)
    }
}

fn temp_output(extension: &'static str) -> Encoded {
    let name = format!("yomine-{}.{extension}", uuid::Uuid::new_v4().simple());
    Encoded { path: std::env::temp_dir().join(name), extension }
}

/// Deletes clips a quit or crash left in the temp folder; normally they go when dropped.
pub fn remove_leftovers() {
    let Ok(dir) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for entry in dir.flatten() {
        if is_clip_name(&entry.file_name().to_string_lossy()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Only the names `temp_output` makes.
fn is_clip_name(name: &str) -> bool {
    let Some((id, extension)) = name.strip_prefix("yomine-").and_then(|n| n.split_once('.')) else {
        return false;
    };
    id.len() == 32
        && id.bytes().all(|b| b.is_ascii_hexdigit())
        && matches!(extension, "mp3" | "ogg" | "jpg" | "png")
}

fn last_line(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no output").trim().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn leftover_cleanup_matches_only_clip_names() {
        assert!(super::is_clip_name("yomine-0013153c66ba4c09a1d242275aa85e54.mp3"));
        assert!(super::is_clip_name("yomine-065a09c0f0c14f47affb4b066815aea4.jpg"));
        assert!(!super::is_clip_name("yomine-sidecars-0013153c66ba4c09a1d242275aa85e54"));
        assert!(!super::is_clip_name("yomine-notes.jpg"));
        assert!(!super::is_clip_name("yomine-0013153c66ba4c09a1d242275aa85e54.srt"));
    }
}
