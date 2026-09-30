//! Sentence audio and a screenshot for a note that already exists.

use std::{
    collections::HashMap,
    path::{
        Path,
        PathBuf,
    },
    sync::{
        Arc,
        Mutex,
    },
    time::Duration,
};

use base64::Engine;
use tauri::{
    async_runtime::JoinHandle,
    ipc::Channel,
};
use tokio::sync::OnceCell;
use yomine::{
    anki::{
        self,
        FieldMapping,
    },
    core::settings::MiningMode,
    media::{
        clip::MediaFormat,
        ffmpeg::{
            self,
            Encoded,
        },
        probe::MediaInfo,
    },
};

use crate::{
    batches::{
        Failure,
        FailureKind,
        FailureScope,
    },
    dto::TimeStampDto,
    events::LoadingMessage,
    player_task::PlayerHandle,
    state::AppState,
};

const SEEK_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);
const SEEK_CONFIRM_POLL: Duration = Duration::from_millis(250);
/// Extra wait past the cue's duration for asbplayer to finish recording.
const RECORD_BUFFER: Duration = Duration::from_millis(1500);
const MEDIA_VERIFY_TIMEOUT: Duration = Duration::from_secs(6);
const MEDIA_VERIFY_POLL: Duration = Duration::from_millis(500);

pub enum MediaSource {
    Asbplayer {
        media_id: Option<String>,
    },
    LocalFile {
        video: PathBuf,
        ffmpeg_path: String,
        format: MediaFormat,
        mappings: HashMap<String, FieldMapping>,
    },
}

/// The fields a note type's media go in; `None` when neither is chosen.
pub fn media_fields(mapping: Option<&FieldMapping>) -> Option<(Option<&str>, Option<&str>)> {
    fn field(f: &Option<String>) -> Option<&str> {
        f.as_deref().filter(|f| !f.is_empty())
    }
    let mapping = mapping?;
    let fields = (field(&mapping.sentence_audio_field), field(&mapping.picture_field));
    (fields.0.is_some() || fields.1.is_some()).then_some(fields)
}

impl MediaSource {
    /// The source the mining mode setting picks for the loaded file. `asbplayer_target` is
    /// the tab to record from when the file didn't come from asbplayer.
    pub fn for_file(state: &AppState, asbplayer_target: Option<String>) -> Result<Self, String> {
        match state.settings.mining_mode {
            MiningMode::Asbplayer => Ok(Self::Asbplayer {
                media_id: state.file.asbplayer_media_id.clone().or(asbplayer_target),
            }),
            MiningMode::Local => Ok(Self::LocalFile {
                video: state
                    .file
                    .local_video
                    .clone()
                    .ok_or("Pair a video with this file to add audio and a screenshot")?,
                ffmpeg_path: state.settings.ffmpeg_path.clone(),
                format: state.settings.media_format.clone(),
                mappings: state.settings.anki_model_mappings.clone(),
            }),
        }
    }

    /// Checked before a batch records an item, so it pauses instead of failing each one.
    pub async fn validate(&self, player: &PlayerHandle) -> Result<(), Failure> {
        let media_id = match self {
            Self::Asbplayer { media_id } => media_id,
            Self::LocalFile { video, ffmpeg_path, .. } => {
                return validate_local(video, ffmpeg_path).await
            }
        };
        let status = player
            .status()
            .await
            .map_err(|e| Failure::new("asbplayer", FailureScope::Shared, e))?;
        if status.ws_clients == 0 {
            return Err(Failure::new(
                "asbplayer",
                FailureScope::Shared,
                "asbplayer is disconnected. Open the video and reconnect the extension, then retry.",
            ));
        }
        let media = player
            .get_bound_media()
            .await
            .map_err(|e| Failure::new("asbplayer", FailureScope::Unknown, e))?;
        let target =
            media.iter().find(|m| Some(m.id.as_str()) == media_id.as_deref()).ok_or_else(|| {
                Failure::new(
                    "asbplayer",
                    FailureScope::Shared,
                    "The original video is not available. Reopen it in asbplayer before retrying.",
                )
            })?;
        if !target.active || target.loaded_subtitles.is_empty() {
            return Err(Failure::new(
                "asbplayer",
                FailureScope::Shared,
                "Activate the video's tab and load its subtitles in asbplayer, then retry.",
            ));
        }
        Ok(())
    }

    pub async fn attach(
        &self,
        player: &PlayerHandle,
        note_id: u64,
        timestamp: Option<&TimeStampDto>,
        progress: &Channel<LoadingMessage>,
    ) -> Result<Option<Preview>, EnrichError> {
        match self {
            Self::Asbplayer { media_id } => {
                enrich_and_verify(player, note_id, media_id.clone(), timestamp, progress)
                    .await
                    .map(|file| file.map(Preview::AnkiFile))
            }
            Self::LocalFile { video, ffmpeg_path, format, mappings } => {
                let timestamp =
                    timestamp.ok_or("This line has no timestamp to cut media from".to_string())?;
                let _ = progress.send(LoadingMessage::new("Cutting audio & screenshot…"));
                attach_local(note_id, timestamp, video, ffmpeg_path, format, mappings).await
            }
        }
    }
}

const NO_FFMPEG: &str =
    "ffmpeg wasn't found. Install it, or download it in Settings → Local Media, then retry.";

async fn validate_local(video: &Path, ffmpeg_path: &str) -> Result<(), Failure> {
    if !video.is_file() {
        return Err(Failure::new(
            "Local video",
            FailureScope::Shared,
            format!("{} no longer exists. Pair the video again, then retry.", video.display()),
        ));
    }
    let configured = ffmpeg_path.to_string();
    let found = tauri::async_runtime::spawn_blocking(move || find_ffmpeg(&configured).is_some())
        .await
        .unwrap_or(false);
    if !found {
        return Err(Failure::new("ffmpeg", FailureScope::Shared, NO_FFMPEG));
    }
    Ok(())
}

fn find_ffmpeg(configured: &str) -> Option<ffmpeg::Ffmpeg> {
    ffmpeg::find(Some(configured.trim()).filter(|p| !p.is_empty()).map(Path::new))
}

async fn attach_local(
    note_id: u64,
    timestamp: &TimeStampDto,
    video: &Path,
    ffmpeg_path: &str,
    format: &MediaFormat,
    mappings: &HashMap<String, FieldMapping>,
) -> Result<Option<Preview>, EnrichError> {
    let anki = anki::current();
    let note = anki
        .notes(&[note_id])
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or("Anki no longer has this note".to_string())?;
    let (audio_field, picture_field) =
        media_fields(mappings.get(&note.note_type)).ok_or_else(|| {
            format!(
                "Choose where {} keeps sentence audio and screenshots in Anki Settings",
                note.note_type
            )
        })?;

    let cutter = cutter_for(video, ffmpeg_path, format);
    let clips = cutter.clips(timestamp).await?;
    let read = |file: &Result<Encoded, String>| {
        let file = file.as_ref().map_err(String::clone)?;
        std::fs::read(&file.path).map(|bytes| (bytes, file.extension)).map_err(|e| e.to_string())
    };
    let wanted = |field: Option<&str>, file| field.map(|_| read(file)).transpose();
    let encoded = match (wanted(audio_field, &clips.audio), wanted(picture_field, &clips.picture)) {
        (Ok(audio), Ok(picture)) => (audio, picture),
        (Err(error), _) | (_, Err(error)) => {
            cutter.forget(timestamp);
            return Err(error.into());
        }
    };

    let mut fields: HashMap<String, String> = HashMap::new();
    let mut preview = None;
    for (field, file, is_picture) in
        [(audio_field, encoded.0, false), (picture_field, encoded.1, true)]
    {
        let (Some(field), Some((bytes, extension))) = (field, file) else { continue };
        let name = format!("yomine-{note_id}.{extension}");
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        match anki.store_media(&name, &data).await {
            Ok(_) => {}
            Err(anki::AnkiError::Rejected(error)) => {
                return Err(EnrichError::Failed(format!("Anki didn't store {name}: {error}")))
            }
            Err(e) => return Err(e.to_string().into()),
        }
        let value =
            if is_picture { format!("<img src=\"{name}\">") } else { format!("[sound:{name}]") };
        // Both can go in one field.
        fields.entry(field.to_string()).or_default().push_str(&value);
        if is_picture {
            preview = Some(Preview::DataUri(data_uri(extension, &data)));
        }
    }
    anki.update_fields(note_id, &fields).await.map_err(|e| e.to_string())?;
    Ok(preview)
}

/// The running batch's clips, cut ahead of its record phase.
static PREPARED: Mutex<Option<Prepared>> = Mutex::new(None);

struct Prepared {
    cutter: Arc<Cutter>,
    worker: JoinHandle<()>,
}

/// Cuts every cue's media in the background, in order, keeping clips already cut for the
/// same video and format.
pub fn prepare(source: &MediaSource, cues: Vec<TimeStampDto>) {
    let MediaSource::LocalFile { video, ffmpeg_path, format, .. } = source else { return };
    let mut prepared = PREPARED.lock().unwrap();
    let cutter = prepared
        .as_ref()
        .map(|p| p.cutter.clone())
        .filter(|c| c.serves(video, ffmpeg_path, format))
        .unwrap_or_else(|| Arc::new(Cutter::new(video, ffmpeg_path, format)));
    let worker = tauri::async_runtime::spawn({
        let cutter = cutter.clone();
        async move {
            for cue in cues {
                let _ = cutter.clips(&cue).await;
            }
        }
    });
    if let Some(earlier) = prepared.replace(Prepared { cutter, worker }) {
        earlier.worker.abort();
    }
}

#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LineMedia {
    Frame,
    Audio,
}

/// As a `data:` URI, from the prepared cutter's clips when it has the line.
pub async fn line_media(
    source: &MediaSource,
    cue: &TimeStampDto,
    kind: LineMedia,
) -> Result<Option<String>, String> {
    let MediaSource::LocalFile { video, ffmpeg_path, format, .. } = source else { return Ok(None) };
    let clips = cutter_for(video, ffmpeg_path, format).clips(cue).await?;
    let file = match kind {
        LineMedia::Frame => &clips.picture,
        LineMedia::Audio => &clips.audio,
    };
    let Ok(file) = file else { return Ok(None) };
    let bytes = std::fs::read(&file.path).map_err(|e| e.to_string())?;
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(data_uri(file.extension, &data)))
}

fn data_uri(extension: &str, base64: &str) -> String {
    let mime = match extension {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        _ => "application/octet-stream",
    };
    format!("data:{mime};base64,{base64}")
}

/// Stops the worker and deletes clips no one is using.
pub fn stop_preparing() {
    if let Some(prepared) = PREPARED.lock().unwrap().take() {
        prepared.worker.abort();
    }
}

fn cutter_for(video: &Path, ffmpeg_path: &str, format: &MediaFormat) -> Arc<Cutter> {
    let prepared = PREPARED.lock().unwrap().as_ref().map(|p| p.cutter.clone());
    prepared
        .filter(|c| c.serves(video, ffmpeg_path, format))
        .unwrap_or_else(|| Arc::new(Cutter::new(video, ffmpeg_path, format)))
}

type CueKey = (u32, u32);

/// Cuts each cue of one video once. The worker and the record step share it, so a cue the
/// worker is still encoding is waited for rather than encoded twice.
struct Cutter {
    video: PathBuf,
    ffmpeg_path: String,
    format: MediaFormat,
    tool: OnceCell<Arc<(ffmpeg::Ffmpeg, MediaInfo)>>,
    clips: Mutex<HashMap<CueKey, Arc<OnceCell<Arc<Clips>>>>>,
}

/// Both are cut whatever the note type needs, since the worker runs before notes exist.
struct Clips {
    audio: Result<Encoded, String>,
    picture: Result<Encoded, String>,
}

impl Cutter {
    fn new(video: &Path, ffmpeg_path: &str, format: &MediaFormat) -> Self {
        Self {
            video: video.to_path_buf(),
            ffmpeg_path: ffmpeg_path.to_string(),
            format: format.clone(),
            tool: OnceCell::new(),
            clips: Mutex::default(),
        }
    }

    fn serves(&self, video: &Path, ffmpeg_path: &str, format: &MediaFormat) -> bool {
        self.video == video && self.ffmpeg_path == ffmpeg_path && &self.format == format
    }

    fn key(cue: &TimeStampDto) -> CueKey {
        (cue.start_secs.to_bits(), cue.end_secs.to_bits())
    }

    async fn clips(&self, cue: &TimeStampDto) -> Result<Arc<Clips>, String> {
        let tool = self
            .tool
            .get_or_try_init(|| {
                let (configured, video) = (self.ffmpeg_path.clone(), self.video.clone());
                async move {
                    tauri::async_runtime::spawn_blocking(move || {
                        let ff = find_ffmpeg(&configured).ok_or(NO_FFMPEG.to_string())?;
                        let info = ff.probe(&video).map_err(|e| e.to_string())?;
                        Ok::<_, String>(Arc::new((ff, info)))
                    })
                    .await
                    .map_err(|e| e.to_string())?
                }
            })
            .await?
            .clone();
        let cell = self.clips.lock().unwrap().entry(Self::key(cue)).or_default().clone();
        let (video, format) = (self.video.clone(), self.format.clone());
        let range = (f64::from(cue.start_secs), f64::from(cue.end_secs));
        cell.get_or_try_init(|| async move {
            tauri::async_runtime::spawn_blocking(move || {
                let (ff, info) = &*tool;
                let error = |e: yomine::core::YomineError| e.to_string();
                Arc::new(Clips {
                    audio: ff.encode_audio(&video, info, range, &format).map_err(error),
                    picture: ff.encode_frame(&video, info, range, &format).map_err(error),
                })
            })
            .await
            .map_err(|e| e.to_string())
        })
        .await
        .cloned()
    }

    /// Drops a cue's clips so the next attempt cuts them again.
    fn forget(&self, cue: &TimeStampDto) {
        self.clips.lock().unwrap().remove(&Self::key(cue));
    }
}

/// The screenshot a record step added, for the batch preview.
pub enum Preview {
    /// A file in Anki's media folder.
    AnkiFile(String),
    /// The image just cut, so showing it needs no Anki request.
    DataUri(String),
}

pub enum EnrichError {
    Unverified,
    Failed(String),
}

impl From<String> for EnrichError {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}

impl std::fmt::Display for EnrichError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unverified => f.write_str(
                "asbplayer didn't update the card. In a new tab, audio recording usually has to be \
                 enabled first: open the video tab, click the asbplayer button in the browser \
                 toolbar and allow recording, then retry. If recording is already enabled, check \
                 asbplayer's Anki settings (deck, note type, and field mappings).",
            ),
            Self::Failed(error) => f.write_str(error),
        }
    }
}

impl EnrichError {
    pub fn failure(&self) -> Failure {
        let failure = Failure::new("Recording media", FailureScope::Unknown, self);
        match self {
            Self::Unverified => failure.with_kind(FailureKind::MediaUnverified),
            Self::Failed(_) => failure,
        }
    }
}

/// The note's current field values, or `None` when Anki can't serve it.
async fn snapshot_fields(note_id: u64) -> Option<HashMap<String, String>> {
    let notes = anki::current().notes(&[note_id]).await.ok()?;
    Some(notes.into_iter().next()?.fields)
}

/// Seek, mine, then verify the enrichment actually changed the note: asbplayer's
/// `published: true` only means the command was broadcast — recording and the
/// note update happen asynchronously afterwards. Verification also catches a
/// pre-v1.20 extension ignoring `noteId` and updating the last-added note.
async fn enrich_and_verify(
    player: &PlayerHandle,
    note_id: u64,
    media_id: Option<String>,
    timestamp: Option<&TimeStampDto>,
    progress: &Channel<LoadingMessage>,
) -> Result<Option<String>, EnrichError> {
    let _ = progress.send(LoadingMessage::new("Adding audio & screenshot via asbplayer…"));
    let baseline = snapshot_fields(note_id).await;

    if let Some(t) = timestamp {
        player.seek(t.start_secs, t.start_label.clone(), media_id.clone()).await?;
        wait_for_seek_confirmation(player, t.start_secs).await;
    }
    player.mine_subtitle(HashMap::new(), 2, media_id, Some(note_id)).await?;

    let Some(baseline) = baseline else {
        return Err(EnrichError::Failed(
            "Recording was requested, but Anki could not be read to verify the media".into(),
        ));
    };

    let _ = progress.send(LoadingMessage::new("Waiting for asbplayer to record the cue…"));
    let record_secs = timestamp.map_or(0.0, |t| (t.end_secs - t.start_secs).max(0.0));
    tokio::time::sleep(Duration::from_secs_f32(record_secs) + RECORD_BUFFER).await;

    let _ = progress.send(LoadingMessage::new("Verifying the media landed in Anki…"));
    let deadline = std::time::Instant::now() + MEDIA_VERIFY_TIMEOUT;
    loop {
        if let Some(now) = snapshot_fields(note_id).await.filter(|now| *now != baseline) {
            return Ok(new_image(&baseline, &now));
        }
        if std::time::Instant::now() >= deadline {
            return Err(EnrichError::Unverified);
        }
        tokio::time::sleep(MEDIA_VERIFY_POLL).await;
    }
}

fn new_image(before: &HashMap<String, String>, after: &HashMap<String, String>) -> Option<String> {
    after.iter().find_map(|(field, value)| {
        let old = before.get(field).map(String::as_str).unwrap_or_default();
        image_sources(value).into_iter().find(|src| !old.contains(src)).map(str::to_string)
    })
}

fn image_sources(html: &str) -> Vec<&str> {
    html.split("<img")
        .skip(1)
        .filter_map(|tag| {
            let src = tag.split('>').next()?.split_once("src=")?.1;
            match src.chars().next()? {
                quote @ ('"' | '\'') => src[1..].split(quote).next(),
                _ => src.split(char::is_whitespace).next(),
            }
        })
        .filter(|src| !src.is_empty())
        .collect()
}

/// Best-effort seek-ack wait so asbplayer records the right cue; proceeds on
/// timeout rather than failing.
async fn wait_for_seek_confirmation(player: &PlayerHandle, secs: f32) {
    let deadline = std::time::Instant::now() + SEEK_CONFIRM_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if let Ok(status) = player.status().await {
            if status.confirmed_timestamps.iter().any(|t| (t - secs).abs() < 0.01) {
                return;
            }
        }
        tokio::time::sleep(SEEK_CONFIRM_POLL).await;
    }
}
