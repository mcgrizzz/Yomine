//! Sentence audio and a screenshot for a note that already exists.

use std::{
    collections::HashMap,
    time::Duration,
};

use tauri::ipc::Channel;
use yomine::anki::api as anki_api;

use crate::{
    batches::{
        Failure,
        FailureKind,
        FailureScope,
    },
    dto::TimeStampDto,
    events::LoadingMessage,
    player_task::PlayerHandle,
};

const SEEK_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);
const SEEK_CONFIRM_POLL: Duration = Duration::from_millis(250);
/// Extra wait past the cue's duration for asbplayer to finish recording.
const RECORD_BUFFER: Duration = Duration::from_millis(1500);
const MEDIA_VERIFY_TIMEOUT: Duration = Duration::from_secs(6);
const MEDIA_VERIFY_POLL: Duration = Duration::from_millis(500);

pub enum MediaSource {
    Asbplayer { media_id: Option<String> },
}

impl MediaSource {
    /// Checked before a batch records an item, so it pauses instead of failing each one.
    pub async fn validate(&self, player: &PlayerHandle) -> Result<(), Failure> {
        let Self::Asbplayer { media_id } = self;
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

    /// asbplayer's `mine-subtitle` drops targets without loaded subtitles, so
    /// enriching against one can only fail — detect it up front. Unknown states
    /// (no target id, pre-v1.20 extension) fall through to the normal attempt.
    pub async fn lacks_subtitles(&self, player: &PlayerHandle) -> bool {
        let Self::Asbplayer { media_id } = self;
        let Some(id) = media_id else { return false };
        let Ok(media) = player.get_bound_media().await else { return false };
        !media.iter().any(|m| &m.id == id && !m.loaded_subtitles.is_empty())
    }

    /// Returns the new screenshot's filename, for the batch preview.
    pub async fn attach(
        &self,
        player: &PlayerHandle,
        note_id: u64,
        timestamp: Option<&TimeStampDto>,
        progress: &Channel<LoadingMessage>,
    ) -> Result<Option<String>, EnrichError> {
        let Self::Asbplayer { media_id } = self;
        enrich_and_verify(player, note_id, media_id.clone(), timestamp, progress).await
    }
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

/// The note's current field values, or `None` when AnkiConnect can't serve it.
async fn snapshot_fields(note_id: u64) -> Option<HashMap<String, String>> {
    let notes = anki_api::get_notes(vec![note_id]).await.ok()?;
    let note = notes.into_iter().next()?;
    Some(note.fields.into_iter().map(|(name, field)| (name, field.value)).collect())
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
