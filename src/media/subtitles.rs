//! Subtitle tracks embedded in a video, extracted to files the loader can open.

use std::{
    ffi::OsString,
    hash::{
        DefaultHasher,
        Hash,
        Hasher,
    },
    path::{
        Path,
        PathBuf,
    },
};

use super::{
    ffmpeg::Ffmpeg,
    probe::{
        is_japanese,
        MediaInfo,
        Stream,
        StreamKind,
    },
};
use crate::{
    core::YomineError,
    persistence::get_shared_file_path,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Extracted {
    /// The file and its number of cues.
    Text {
        path: PathBuf,
        lines: usize,
    },
    /// Bitmap subtitles (PGS, VobSub, DVB), which would need OCR.
    Image,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub stream: Stream,
    pub extracted: Extracted,
}

enum Codec {
    Ass,
    Srt,
    Image,
    Unsupported,
}

fn codec(name: &str) -> Codec {
    match name {
        "ass" | "ssa" => Codec::Ass,
        "subrip" | "srt" | "webvtt" | "mov_text" | "text" => Codec::Srt,
        "hdmv_pgs_subtitle" | "dvd_subtitle" | "dvb_subtitle" | "xsub" => Codec::Image,
        _ => Codec::Unsupported,
    }
}

/// Cached by the video's path, size and modification time, so reopening it skips ffmpeg.
pub fn extract(ffmpeg: &Ffmpeg, video: &Path, info: &MediaInfo) -> Result<Vec<Track>, YomineError> {
    let dir = get_shared_file_path("local_subtitles");
    std::fs::create_dir_all(&dir)?;
    let key = cache_key(video)?;

    let mut tracks = Vec::new();
    // Stream index, output file, and whether the codec can be copied as is.
    let mut outputs: Vec<(usize, PathBuf, bool)> = Vec::new();
    for stream in info.streams.iter().filter(|s| s.kind == StreamKind::Subtitle) {
        let extracted = match codec(&stream.codec) {
            Codec::Ass | Codec::Srt => {
                let copy = matches!(codec(&stream.codec), Codec::Ass);
                let extension = if copy { "ass" } else { "srt" };
                let path = dir.join(format!("{key}-{}.{extension}", stream.index));
                outputs.push((stream.index, path.clone(), copy));
                Extracted::Text { path, lines: 0 }
            }
            Codec::Image => Extracted::Image,
            Codec::Unsupported => Extracted::Unsupported,
        };
        tracks.push(Track { stream: stream.clone(), extracted });
    }

    if outputs.iter().any(|(_, path, _)| !path.exists()) {
        let mut args: Vec<OsString> =
            ["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"].map(Into::into).into();
        args.push(video.into());
        for (index, path, copy) in &outputs {
            args.extend(["-map".into(), format!("0:{index}").into()]);
            args.extend(["-c:s".into(), if *copy { "copy" } else { "srt" }.into()]);
            args.push(path.into());
        }
        if let Err(e) = ffmpeg.run(args) {
            for (_, path, _) in &outputs {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
    }

    for track in &mut tracks {
        if let Extracted::Text { path, lines } = &mut track.extracted {
            *lines = count_cues(&std::fs::read_to_string(&*path).unwrap_or_default());
        }
    }
    Ok(tracks)
}

fn cache_key(video: &Path) -> Result<String, YomineError> {
    let meta = std::fs::metadata(video)?;
    let mut hasher = DefaultHasher::new();
    video.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    meta.modified().ok().hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

fn count_cues(text: &str) -> usize {
    text.lines().filter(|l| l.contains("-->") || l.starts_with("Dialogue:")).count()
}

/// A Japanese track, otherwise an untagged one, otherwise any. Among several, the one
/// with the most cues: a signs track has far fewer than the dialogue.
pub fn default_track(tracks: &[Track]) -> Option<&Track> {
    let best = |keep: &dyn Fn(&Track) -> bool| {
        tracks
            .iter()
            .filter_map(|t| match t.extracted {
                Extracted::Text { lines, .. } if keep(t) => Some((lines, t)),
                _ => None,
            })
            .max_by_key(|(lines, t)| (*lines, std::cmp::Reverse(t.stream.index)))
            .map(|(_, t)| t)
    };
    best(&|t| is_japanese(t.stream.language.as_deref()))
        .or_else(|| best(&|t| t.stream.language.is_none()))
        .or_else(|| best(&|_| true))
}

#[cfg(test)]
mod tests {
    use super::{
        super::probe,
        *,
    };

    fn tracks(lines: &[usize]) -> Vec<Track> {
        let info = probe::parse(include_str!(
            "../../tests/fixtures/ffmpeg/mkv_signs_and_dialogue_4.4.txt"
        ));
        let subtitles = info.streams.into_iter().filter(|s| s.kind == StreamKind::Subtitle);
        subtitles
            .zip(lines)
            .map(|(stream, &lines)| Track {
                stream,
                extracted: Extracted::Text { path: PathBuf::new(), lines },
            })
            .collect()
    }

    #[test]
    fn dialogue_beats_signs() {
        // Signs, Dialogue (both Japanese), then English.
        let tracks = tracks(&[2, 5, 40]);
        assert_eq!(
            default_track(&tracks).and_then(|t| t.stream.title.as_deref()),
            Some("Dialogue")
        );
    }
}
