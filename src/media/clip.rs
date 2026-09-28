//! What to cut from a video for one cue, as ffmpeg arguments.

use std::{
    ffi::OsString,
    path::Path,
};

use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Mp3,
    Opus,
}

impl AudioFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Opus => "ogg",
        }
    }

    /// Encoders in order of preference; builds include different ones.
    pub fn encoders(self) -> &'static [&'static str] {
        match self {
            Self::Mp3 => &["libmp3lame", "libshine", "mp3_mf"],
            Self::Opus => &["libopus"],
        }
    }

    fn other(self) -> Self {
        match self {
            Self::Mp3 => Self::Opus,
            Self::Opus => Self::Mp3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Jpeg,
    Png,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
}

/// Defaults match asbplayer's, so cards look the same whichever made them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaFormat {
    pub audio: AudioFormat,
    pub image: ImageFormat,
    pub pad_start_ms: u32,
    pub pad_end_ms: u32,
    /// 0 means no limit.
    pub max_width: u32,
    pub max_height: u32,
}

impl Default for MediaFormat {
    fn default() -> Self {
        Self {
            audio: AudioFormat::Mp3,
            image: ImageFormat::Jpeg,
            pad_start_ms: 0,
            pad_end_ms: 500,
            max_width: 0,
            max_height: 0,
        }
    }
}

pub fn pick_audio_encoder(
    preferred: AudioFormat,
    available: impl Fn(&str) -> bool,
) -> Option<(AudioFormat, &'static str)> {
    [preferred, preferred.other()].into_iter().find_map(|format| {
        format.encoders().iter().find(|e| available(e)).map(|encoder| (format, *encoder))
    })
}

/// Start and length in seconds, padded and kept inside the video.
pub fn audio_range(
    cue_start: f64,
    cue_end: f64,
    format: &MediaFormat,
    duration: Option<f64>,
) -> (f64, f64) {
    let start = (cue_start - f64::from(format.pad_start_ms) / 1000.0).max(0.0);
    let mut end = cue_end + f64::from(format.pad_end_ms) / 1000.0;
    if let Some(duration) = duration {
        end = end.min(duration);
    }
    (start, (end - start).max(0.0))
}

/// Mid-cue, which is less likely than the start to land on a scene cut.
pub fn frame_time(cue_start: f64, cue_end: f64, duration: Option<f64>) -> f64 {
    let middle = (cue_start + cue_end.max(cue_start)) / 2.0;
    // A seek to the very end decodes no frame.
    duration.map_or(middle, |d| middle.min((d - 0.1).max(0.0)))
}

fn common(start: f64, input: &Path, stream: usize) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-ss"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.push(format!("{start:.3}").into());
    args.push("-i".into());
    args.push(input.into());
    args.push("-map".into());
    args.push(format!("0:{stream}").into());
    args
}

pub fn audio_args(
    input: &Path,
    stream: usize,
    (start, length): (f64, f64),
    encoder: &str,
    output: &Path,
) -> Vec<OsString> {
    let mut args = common(start, input, stream);
    for arg in ["-t", &format!("{length:.3}"), "-vn", "-sn", "-dn", "-c:a", encoder] {
        args.push(arg.into());
    }
    args.push("-b:a".into());
    args.push(if encoder == "libopus" { "64k" } else { "128k" }.into());
    args.push(output.into());
    args
}

pub fn frame_args(
    input: &Path,
    stream: usize,
    time: f64,
    format: &MediaFormat,
    output: &Path,
) -> Vec<OsString> {
    let mut args = common(time, input, stream);
    args.push("-frames:v".into());
    args.push("1".into());
    if format.max_width > 0 || format.max_height > 0 {
        let limit = |max: u32, side: &str| {
            if max > 0 {
                format!("'min({side},{max})'")
            } else {
                side.to_string()
            }
        };
        args.push("-vf".into());
        args.push(
            format!(
                "scale=w={}:h={}:force_original_aspect_ratio=decrease",
                limit(format.max_width, "iw"),
                limit(format.max_height, "ih")
            )
            .into(),
        );
    }
    if format.image == ImageFormat::Jpeg {
        args.push("-q:v".into());
        args.push("2".into());
    }
    args.push(output.into());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_stays_inside_the_video() {
        let format = MediaFormat { pad_start_ms: 300, pad_end_ms: 500, ..Default::default() };
        assert_eq!(audio_range(0.1, 2.0, &format, Some(60.0)), (0.0, 2.5));
        let (start, length) = audio_range(58.0, 59.8, &format, Some(60.0));
        assert!((start - 57.7).abs() < 1e-9 && (length - 2.3).abs() < 1e-9);
        assert!((audio_range(5.0, 6.0, &format, None).1 - 1.8).abs() < 1e-9);
    }

    #[test]
    fn a_cue_past_the_end_still_gets_a_frame() {
        assert_eq!(frame_time(10.0, 12.0, Some(60.0)), 11.0);
        assert!(frame_time(70.0, 72.0, Some(60.0)) < 60.0);
    }
}
