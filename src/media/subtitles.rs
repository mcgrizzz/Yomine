//! A video's subtitles: files beside it with the same name, and text tracks embedded in it,
//! extracted to files the loader can open.

use std::{
    cmp::Ordering,
    collections::HashSet,
    ffi::OsString,
    fs::Metadata,
    hash::{
        DefaultHasher,
        Hash,
        Hasher,
    },
    iter::Peekable,
    path::{
        Path,
        PathBuf,
    },
    str::Chars,
};

use serde::{
    Deserialize,
    Serialize,
};

use super::{
    ffmpeg::Ffmpeg,
    probe::{
        is_japanese,
        MediaInfo,
        StreamKind,
    },
};
use crate::{
    core::{
        filename_parser::{
            parse_filename,
            MediaType,
        },
        YomineError,
    },
    persistence::get_shared_file_path,
};

pub const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "webm", "mov", "m4v", "ts"];
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleChoice {
    /// Unique within one video: `file:<name>` or `stream:<index>`.
    pub id: String,
    pub label: String,
    pub language: Option<String>,
    pub lines: usize,
    /// `None` for a track that can't be loaded, with the reason in `unusable`.
    pub path: Option<PathBuf>,
    pub unusable: Option<String>,
}

/// Japanese or untagged: what can be mined. Other languages are left out.
pub fn is_candidate(choice: &SubtitleChoice) -> bool {
    choice.language.is_none() || is_japanese(choice.language.as_deref())
}

/// The languages of subtitles that aren't candidates, to say what a video does have.
pub fn other_languages(choices: &[SubtitleChoice]) -> Vec<String> {
    let mut names: Vec<String> = choices
        .iter()
        .filter(|c| !is_candidate(c))
        .map(|c| language_name(c.language.as_deref()))
        .collect();
    names.dedup();
    names
}

pub fn is_video(path: &Path) -> bool {
    extension(path).is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.as_str()))
}

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

const TAG_FLAGS: &[&str] = &["forced", "sdh", "cc", "hi", "signs", "songs", "full", "default"];

/// ffmpeg's three-letter code for a language as subtitle names write it.
fn language_code(part: &str) -> Option<&'static str> {
    Some(match part {
        "ja" | "jp" | "jpn" | "jap" | "japanese" => "jpn",
        "en" | "eng" | "english" => "eng",
        "es" | "spa" => "spa",
        "fr" | "fre" | "fra" => "fra",
        "de" | "ger" | "deu" => "deu",
        "it" | "ita" => "ita",
        "pt" | "por" => "por",
        "ru" | "rus" => "rus",
        "ko" | "kor" => "kor",
        "zh" | "chi" | "zho" => "zho",
        _ => return None,
    })
}

/// Whether one dotted part of a name is a language tag: `ja`, `ja-JP`, `ja[cc]`, `forced`.
fn is_language_tag(part: &str) -> bool {
    let part = part.to_ascii_lowercase();
    let (part, bracketed) = match part.strip_suffix(']').and_then(|p| p.split_once('[')) {
        Some((part, flag)) => (part.to_string(), Some(flag.to_string())),
        None => (part, None),
    };
    let mut pieces = part.split(['-', '_']);
    let first = pieces.next().unwrap_or_default();
    (language_code(first).is_some() || TAG_FLAGS.contains(&first))
        && pieces.all(|p| p.len() == 2 || TAG_FLAGS.contains(&p))
        && bracketed.is_none_or(|f| TAG_FLAGS.contains(&f.as_str()))
}

/// A subtitle file's stem without its language tag: `ep01` for `ep01.jpn.forced`.
pub fn strip_language_tag(stem: &str) -> &str {
    let mut base = stem;
    while let Some((rest, part)) = base.rsplit_once('.') {
        if rest.is_empty() || !is_language_tag(part) {
            break;
        }
        base = rest;
    }
    base
}

/// The language tag in `ep01.ja.srt`, normalised to ffmpeg's three-letter codes.
/// Tags can carry flags too: `jpn.forced`, `ja-JP`, `en.sdh`.
fn tag_language(tag: &str) -> (Option<String>, Vec<String>) {
    let mut language = None;
    let mut flags = Vec::new();
    for part in tag.split(['.', '_', '-']).map(str::to_ascii_lowercase).filter(|p| !p.is_empty()) {
        match language_code(&part) {
            Some(code) if language.is_none() => language = Some(code.to_string()),
            Some(_) => {}
            None if TAG_FLAGS.contains(&part.as_str()) => flags.push(part),
            // A region after the language, as in ja-JP.
            None if language.is_some() => {}
            None => language = Some(part),
        }
    }
    (language, flags)
}

/// `ep01.srt` and `ep01.ja.ass` for `ep01.mkv`.
pub fn sidecars(video: &Path) -> Vec<SubtitleChoice> {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem().and_then(|s| s.to_str()))
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<SubtitleChoice> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.to_string();
            let tag = sidecar_tag(stem, &path)?;
            let lines = count_cues(&std::fs::read_to_string(&path).unwrap_or_default());
            let (language, flags) = tag_language(tag);
            let kind = extension(&path).unwrap_or_default();
            let spoken = language.as_deref().map(|l| language_name(Some(l)));
            let described: Vec<String> = spoken.into_iter().chain(flags).collect();
            let label = if described.is_empty() {
                format!("{kind} file")
            } else {
                format!("{} ({kind} file)", described.join(" "))
            };
            Some(SubtitleChoice {
                id: format!("file:{name}"),
                label,
                language,
                lines,
                path: Some(path),
                unusable: None,
            })
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// The language tag of a subtitle file named for the video `stem`: `ja` for `ep01.ja.srt`,
/// empty for `ep01.srt`.
fn sidecar_tag<'a>(stem: &str, path: &'a Path) -> Option<&'a str> {
    if !extension(path).is_some_and(|e| SUBTITLE_EXTENSIONS.contains(&e.as_str())) {
        return None;
    }
    let sub_stem = path.file_stem()?.to_str()?;
    // Subtitles from another release can differ from the video's name in case alone, which
    // Windows treats as the same name.
    if !sub_stem.get(..stem.len())?.eq_ignore_ascii_case(stem) {
        return None;
    }
    let tag = &sub_stem[stem.len()..];
    if tag.is_empty() {
        Some(tag)
    } else {
        tag.strip_prefix('.')
    }
}

/// The video beside a subtitle file whose name it starts with, the longest where several do.
pub fn sibling_video(subtitle: &Path) -> Option<PathBuf> {
    std::fs::read_dir(subtitle.parent()?)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| is_video(path))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?;
            sidecar_tag(stem, subtitle).map(|_| (stem.len(), path.clone()))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, path)| path)
}

/// A folder's videos, and its Japanese or untagged subtitle files that no video there pairs
/// with: what opening one of its episodes would load.
pub fn episodes_in(dir: &Path) -> Vec<(PathBuf, Metadata)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    // A directory listing on Windows carries each file's metadata, so this asks for nothing
    // more of a network share.
    let files: Vec<(PathBuf, Metadata)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok().filter(Metadata::is_file).map(|meta| (e.path(), meta)))
        .collect();
    let videos: Vec<&str> = files
        .iter()
        .map(|(path, _)| path)
        .filter(|path| is_video(path))
        .filter_map(|path| path.file_stem()?.to_str())
        .collect();
    let unpaired_japanese = |path: &Path| {
        if !extension(path).is_some_and(|e| SUBTITLE_EXTENSIONS.contains(&e.as_str())) {
            return false;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { return false };
        let (language, _) = tag_language(&stem[strip_language_tag(stem).len()..]);
        !videos.iter().any(|video| sidecar_tag(video, path).is_some())
            && (language.is_none() || is_japanese(language.as_deref()))
    };
    files.iter().filter(|(path, _)| is_video(path) || unpaired_japanese(path)).cloned().collect()
}

/// Whether a Japanese or untagged subtitle file sits beside the video.
pub fn has_minable_beside(video: &Path) -> bool {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem().and_then(|s| s.to_str()))
    else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.filter_map(|entry| entry.ok().map(|e| e.path())).any(|path| {
        sidecar_tag(stem, &path).is_some_and(|tag| {
            let (language, _) = tag_language(tag);
            language.is_none() || is_japanese(language.as_deref())
        })
    })
}

/// Whether the video holds a Japanese or untagged text track.
pub fn has_minable_track(video: &Path, ffmpeg: &Ffmpeg) -> Result<bool, YomineError> {
    let tracks = embedded(video, &ffmpeg.probe(video)?)?;
    Ok(tracks.iter().any(|t| is_candidate(t) && t.path.is_some()))
}

/// The videos among `paths`, each folder standing for the videos at its top level, in
/// natural order ("ep2" before "ep10").
pub fn videos_in(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut videos: Vec<PathBuf> = paths
        .iter()
        .flat_map(|path| match std::fs::read_dir(path) {
            Ok(entries) => entries.filter_map(|entry| entry.ok().map(|e| e.path())).collect(),
            Err(_) => vec![path.clone()],
        })
        .filter(|path| path.is_file() && is_video(path))
        .collect();
    videos.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
    videos.dedup();
    for folder in videos.chunk_by_mut(|a, b| a.parent() == b.parent()) {
        order_by_episode(folder);
    }
    videos
}

/// Orders a folder's videos by episode when each is a different one, so a season stays in
/// order when its files are named in different patterns. A folder of several shows repeats
/// episode numbers, and keeps name order.
fn order_by_episode(videos: &mut [PathBuf]) {
    let episode = |video: &PathBuf| match parse_filename(&video.to_string_lossy()) {
        MediaType::TvShow { season, episode: Some(episode), .. } => Some((season, episode)),
        _ => None,
    };
    let episodes: Vec<_> = videos.iter().map(episode).collect();
    let distinct: HashSet<_> = episodes.iter().collect();
    if distinct.contains(&None) || distinct.len() < episodes.len() {
        return;
    }
    videos.sort_by_cached_key(episode);
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        let order = match (a.peek(), b.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (n, m) = (take_number(&mut a), take_number(&mut b));
                n.len().cmp(&m.len()).then_with(|| n.cmp(&m))
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                a.next();
                b.next();
                order
            }
        };
        if order != Ordering::Equal {
            return order;
        }
    }
}

fn take_number(chars: &mut Peekable<Chars>) -> String {
    let mut digits = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        digits.push(c);
    }
    digits.trim_start_matches('0').to_string()
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

fn language_name(code: Option<&str>) -> String {
    match code {
        Some("jpn" | "ja") => "Japanese".into(),
        Some("eng" | "en") => "English".into(),
        Some(other) => other.to_string(),
        None => "Unknown language".into(),
    }
}

/// The video's embedded subtitles as the probe lists them, without extracting any. `lines`
/// is the cue count the muxer recorded, 0 when it didn't, or the real count once a track is
/// extracted. Extracted files are cached by the video's path, size and modification time.
pub fn embedded(video: &Path, info: &MediaInfo) -> Result<Vec<SubtitleChoice>, YomineError> {
    let dir = get_shared_file_path("local_subtitles");
    let key = cache_key(video)?;
    Ok(info
        .streams
        .iter()
        .filter(|s| s.kind == StreamKind::Subtitle)
        .map(|stream| {
            let (path, unusable) = match codec(&stream.codec) {
                Codec::Ass => (Some(dir.join(format!("{key}-{}.ass", stream.index))), None),
                Codec::Srt => (Some(dir.join(format!("{key}-{}.srt", stream.index))), None),
                Codec::Image => (None, Some("Image subtitles can't be mined".to_string())),
                Codec::Unsupported => (None, Some("Unsupported subtitle format".to_string())),
            };
            let lines = match &path {
                Some(path) if path.exists() => {
                    count_cues(&std::fs::read_to_string(path).unwrap_or_default())
                }
                _ => stream.frames.unwrap_or(0),
            };
            let language = language_name(stream.language.as_deref());
            SubtitleChoice {
                id: format!("stream:{}", stream.index),
                label: match &stream.title {
                    Some(title) => format!("{language} ({title})"),
                    None => language,
                },
                language: stream.language.clone(),
                lines,
                path,
                unusable,
            }
        })
        .collect())
}

/// Extracts the given embedded tracks that aren't cached yet, in one ffmpeg run, and counts
/// their cues. ffmpeg reads the whole video to do it, so only the tracks needed are passed.
pub fn extract(
    ffmpeg: &Ffmpeg,
    video: &Path,
    choices: &mut [SubtitleChoice],
) -> Result<(), YomineError> {
    // Stream index, output file, and whether the codec can be copied as is.
    let missing: Vec<(usize, PathBuf, bool)> = choices
        .iter()
        .filter_map(|c| {
            let index = c.id.strip_prefix("stream:")?.parse().ok()?;
            let path = c.path.clone().filter(|p| !p.exists())?;
            let copy = extension(&path).as_deref() == Some("ass");
            Some((index, path, copy))
        })
        .collect();
    if !missing.is_empty() {
        std::fs::create_dir_all(get_shared_file_path("local_subtitles"))?;
        let mut args: Vec<OsString> =
            ["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"].map(Into::into).into();
        args.push(video.into());
        for (index, path, copy) in &missing {
            args.extend(["-map".into(), format!("0:{index}").into()]);
            args.extend(["-c:s".into(), if *copy { "copy" } else { "srt" }.into()]);
            args.push(path.into());
        }
        if let Err(e) = ffmpeg.run(args) {
            for (_, path, _) in &missing {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
    }
    for choice in choices.iter_mut().filter(|c| c.id.starts_with("stream:")) {
        if let Some(path) = &choice.path {
            choice.lines = count_cues(&std::fs::read_to_string(path).unwrap_or_default());
        }
    }
    Ok(())
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

/// A Japanese track, otherwise an untagged one. Among several, the one with the most cues:
/// a signs track has far fewer than the dialogue. Ties go to the earlier choice.
pub fn default_choice(choices: &[SubtitleChoice]) -> Option<&SubtitleChoice> {
    let best = |keep: &dyn Fn(&SubtitleChoice) -> bool| {
        choices
            .iter()
            .enumerate()
            .filter(|(_, c)| c.path.is_some() && keep(c))
            .max_by_key(|(i, c)| (c.lines, std::cmp::Reverse(*i)))
            .map(|(_, c)| c)
    };
    best(&|c| is_japanese(c.language.as_deref())).or_else(|| best(&|c| c.language.is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(id: &str, language: Option<&str>, lines: usize) -> SubtitleChoice {
        SubtitleChoice {
            id: id.into(),
            label: id.into(),
            language: language.map(str::to_string),
            lines,
            path: Some(PathBuf::from(id)),
            unusable: None,
        }
    }

    #[test]
    fn japanese_dialogue_beats_signs_and_other_languages() {
        let choices = [
            choice("stream:3", Some("jpn"), 2),
            choice("stream:4", Some("jpn"), 5),
            choice("stream:5", Some("eng"), 40),
        ];
        assert_eq!(default_choice(&choices).map(|c| c.id.as_str()), Some("stream:4"));
        let untagged = [choice("file:ep01.srt", None, 300), choice("stream:5", Some("eng"), 400)];
        assert_eq!(default_choice(&untagged).map(|c| c.id.as_str()), Some("file:ep01.srt"));
        let foreign = [choice("stream:2", Some("eng"), 400), choice("stream:6", Some("spa"), 380)];
        assert_eq!(default_choice(&foreign), None);
        assert_eq!(other_languages(&foreign), ["English", "spa"]);
    }

    #[test]
    fn finds_subtitles_and_video_by_name() {
        let dir = std::env::temp_dir().join(format!("yomine-sidecars-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "ep01.mkv",
            "ep01.srt",
            "ep01.ja.ass",
            "ep01.en.srt",
            "ep01.jpn.forced.srt",
            "ep01.ja-JP.srt",
            "ep02.srt",
            "ep011.srt",
        ] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let found: Vec<_> =
            sidecars(&dir.join("ep01.mkv")).into_iter().map(|c| (c.id, c.language)).collect();
        assert_eq!(
            found,
            [
                ("file:ep01.en.srt".to_string(), Some("eng".to_string())),
                ("file:ep01.ja-JP.srt".to_string(), Some("jpn".to_string())),
                ("file:ep01.ja.ass".to_string(), Some("jpn".to_string())),
                ("file:ep01.jpn.forced.srt".to_string(), Some("jpn".to_string())),
                ("file:ep01.srt".to_string(), None),
            ]
        );
        assert_eq!(sibling_video(&dir.join("ep01.ja.ass")), Some(dir.join("ep01.mkv")));
        assert_eq!(sibling_video(&dir.join("ep01.jpn.forced.srt")), Some(dir.join("ep01.mkv")));
        assert_eq!(sibling_video(&dir.join("ep02.srt")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn subtitles_pair_with_a_video_named_in_other_case() {
        let dir = std::env::temp_dir().join(format!("yomine-case-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in
            ["Show.S01e04.Bluray.mp4", "Show.S01E04.BluRay.jpn.srt", "Show.S01E04.BluRay.jpn.0.srt"]
        {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let video = dir.join("Show.S01e04.Bluray.mp4");
        assert_eq!(sidecars(&video).len(), 2);
        assert_eq!(sibling_video(&dir.join("Show.S01E04.BluRay.jpn.0.srt")), Some(video.clone()));
        let episodes: Vec<_> = episodes_in(&dir).into_iter().map(|(path, _)| path).collect();
        assert_eq!(episodes, [video]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folder_lists_its_videos_in_episode_order() {
        let dir = std::env::temp_dir().join(format!("yomine-folder-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("extras")).unwrap();
        for name in ["ep10.mkv", "ep2.mkv", "Ep01.mp4", "ep2.srt", "extras/ep0.mkv"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let names: Vec<_> = videos_in(&[dir.clone(), dir.join("ep2.mkv")])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Ep01.mp4", "ep2.mkv", "ep10.mkv"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_named_in_two_patterns_plays_in_episode_order() {
        let paths = |names: &[&str]| -> Vec<PathBuf> {
            names.iter().map(|name| PathBuf::from("Show/Season 1").join(name)).collect()
        };
        let mut videos = paths(&[
            "S01E13-Serving in the Outer Court [AC984351].mkv",
            "S01E14-The New Pure Consort [70BCC87C].mkv",
            "The.Apothecary.Diaries.S01e01.Maomao.1080P.Bluray.Remux.Avc.Flac.2.0-Crucible.mp4",
            "The.Apothecary.Diaries.S01E12.The.Eunuch.and.the.Courtesan.1080p.BluRay.mkv",
        ]);
        order_by_episode(&mut videos);
        assert_eq!(
            videos,
            paths(&[
                "The.Apothecary.Diaries.S01e01.Maomao.1080P.Bluray.Remux.Avc.Flac.2.0-Crucible.mp4",
                "The.Apothecary.Diaries.S01E12.The.Eunuch.and.the.Courtesan.1080p.BluRay.mkv",
                "S01E13-Serving in the Outer Court [AC984351].mkv",
                "S01E14-The New Pure Consort [70BCC87C].mkv",
            ])
        );

        let mut shows = paths(&["A - 02.mkv", "B - 01.mkv", "B - 02.mkv"]);
        order_by_episode(&mut shows);
        assert_eq!(shows, paths(&["A - 02.mkv", "B - 01.mkv", "B - 02.mkv"]));
    }
}
