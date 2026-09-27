//! Streams and duration read from the listing `ffmpeg -i` prints to stderr. Builds don't
//! always ship ffprobe, and this listing has kept the same shape across versions.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Video,
    Audio,
    Subtitle,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    /// For `-map 0:<index>`.
    pub index: usize,
    pub kind: StreamKind,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    /// Cover art, which files store as a one-frame video stream.
    pub attached_pic: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaInfo {
    pub duration: Option<f64>,
    pub streams: Vec<Stream>,
}

impl MediaInfo {
    pub fn audio_stream(&self) -> Option<&Stream> {
        let mut audio = self.streams.iter().filter(|s| s.kind == StreamKind::Audio);
        audio.clone().find(|s| is_japanese(s.language.as_deref())).or_else(|| audio.next())
    }

    pub fn video_stream(&self) -> Option<&Stream> {
        self.streams.iter().find(|s| s.kind == StreamKind::Video && !s.attached_pic)
    }
}

pub fn is_japanese(language: Option<&str>) -> bool {
    matches!(language, Some("jpn" | "ja"))
}

static STREAM: LazyLock<Regex> = LazyLock::new(|| {
    // `[0x1]` is the container's own stream id, which newer versions print for mp4.
    Regex::new(r"^Stream #0:(\d+)(?:\[[^\]]*\])?(?:\(([^)]*)\))?: (\w+): ([^\s,]+)").unwrap()
});
static DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Duration: (\d+):(\d{2}):(\d{2}(?:\.\d+)?)").unwrap());
static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^title\s*: (.*)$").unwrap());

pub fn parse(stderr: &str) -> MediaInfo {
    let mut info = MediaInfo::default();
    // Metadata belongs to the stream line above it; the file's own metadata comes first.
    let mut in_stream = false;
    for line in stderr.lines() {
        let line = line.trim();
        if line.starts_with("Input #") && !line.starts_with("Input #0") {
            break;
        }
        if let Some(c) = DURATION.captures(line) {
            let part = |i: usize| c[i].parse::<f64>().unwrap_or(0.0);
            info.duration = Some(part(1) * 3600.0 + part(2) * 60.0 + part(3));
        } else if let Some(c) = STREAM.captures(line) {
            in_stream = true;
            info.streams.push(Stream {
                index: c[1].parse().unwrap_or(0),
                kind: match &c[3] {
                    "Video" => StreamKind::Video,
                    "Audio" => StreamKind::Audio,
                    "Subtitle" => StreamKind::Subtitle,
                    _ => StreamKind::Other,
                },
                codec: c[4].to_string(),
                language: c.get(2).map(|l| l.as_str()).filter(|l| *l != "und").map(str::to_string),
                title: None,
                default: line.contains("(default)"),
                attached_pic: line.contains("(attached pic)"),
            });
        } else if line.starts_with("Chapter") {
            in_stream = false;
        } else if let Some(c) = TITLE.captures(line).filter(|_| in_stream) {
            if let Some(stream) = info.streams.last_mut() {
                stream.title.get_or_insert_with(|| c[1].trim().to_string());
            }
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    const MKV: &str = include_str!("../../tests/fixtures/ffmpeg/mkv_signs_and_dialogue_4.4.txt");
    const MP4: &str = include_str!("../../tests/fixtures/ffmpeg/mp4_untagged_6.1.txt");

    #[test]
    fn reads_every_stream_of_an_mkv() {
        let info = parse(MKV);
        assert_eq!(info.duration, Some(10.02));
        let summary: Vec<_> = info
            .streams
            .iter()
            .map(|s| (s.index, s.kind, s.codec.as_str(), s.language.as_deref(), s.title.as_deref()))
            .collect();
        assert_eq!(
            summary,
            [
                (0, StreamKind::Video, "h264", None, None),
                (1, StreamKind::Audio, "aac", Some("eng"), None),
                (2, StreamKind::Audio, "aac", Some("jpn"), Some("日本語")),
                (3, StreamKind::Subtitle, "subrip", Some("jpn"), Some("Signs")),
                (4, StreamKind::Subtitle, "ass", Some("jpn"), Some("Dialogue")),
                (5, StreamKind::Subtitle, "subrip", Some("eng"), None),
            ]
        );
        assert!(info.streams[1].default && !info.streams[2].default);
    }

    #[test]
    fn japanese_audio_wins_over_the_default_track() {
        assert_eq!(parse(MKV).audio_stream().map(|s| s.index), Some(2));
    }

    #[test]
    fn reads_untagged_mp4_streams_from_windows_output() {
        let info = parse(&MP4.replace('\n', "\r\n"));
        assert_eq!(info.duration, Some(10.0));
        let streams: Vec<_> =
            info.streams.iter().map(|s| (s.index, s.kind, s.language.clone())).collect();
        assert_eq!(
            streams,
            [
                (0, StreamKind::Video, None),
                (1, StreamKind::Audio, None),
                (2, StreamKind::Subtitle, None),
            ]
        );
        assert_eq!(info.streams[2].codec, "mov_text");
        assert_eq!(info.audio_stream().map(|s| s.index), Some(1));
    }

    #[test]
    fn recognises_cover_art() {
        let info = parse(
            "  Stream #0:1(und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, mono, fltp, 75 \
             kb/s (default)\n  Stream #0:2: Video: mjpeg (Baseline), yuvj444p(pc, \
             bt470bg/unknown/unknown), 64x64 [SAR 1:1 DAR 1:1], 90k tbr, 90k tbn, 90k tbc \
             (attached pic)\n",
        );
        assert!(info.streams[1].attached_pic);
        assert_eq!(info.video_stream(), None);
    }
}
