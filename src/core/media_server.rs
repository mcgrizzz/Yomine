use serde::Deserialize;
use reqwest::Url;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub index: i32,
    pub title: String,
    pub language: String,
    pub codec: String,
    pub is_default: bool,
    pub is_forced: bool,
    pub download_url: String,
}

#[derive(Debug)]
pub struct MediaServerStream {
    pub base_url: String,
    pub item_id: String,
    pub api_key: String,
    pub media_source_id: Option<String>,
}

impl MediaServerStream {
    pub fn try_parse(url_str: &str) -> Option<Self> {
        let url = Url::parse(url_str).ok()?;
        
        // Find "/Videos/" in the original string to safely extract base_url
        let videos_pos = url_str.find("/Videos/")?;
        
        // base_url includes scheme, host, port, and any path prefix (e.g. /emby)
        let base_url = url_str[..videos_pos].to_string();

        let segments: Vec<&str> = url.path_segments()?.collect();
        let video_idx = segments.iter().position(|&s| s == "Videos")?;
        if video_idx + 1 >= segments.len() {
            return None;
        }
        let item_id = segments[video_idx + 1].to_string();

        let query_pairs: HashMap<_, _> = url.query_pairs().collect();
        let api_key = query_pairs.get("api_key")?.to_string();
        
        // mediaSourceId might be in query params
        let media_source_id = query_pairs.get("mediaSourceId").map(|s| s.to_string());

        Some(Self {
            base_url,
            item_id,
            api_key,
            media_source_id,
        })
    }

    pub fn fetch_subtitles(&self) -> Result<Vec<SubtitleTrack>, String> {
        // Try getting sessions first to find the active playing item which contains stream info
        let endpoint = format!("{}/Sessions?api_key={}", self.base_url, self.api_key);
        println!("[MediaServer] Fetching sessions from: {}", endpoint);
        
        let client = reqwest::blocking::Client::new();
        let resp = client.get(&endpoint).send()
            .map_err(|e| format!("Request failed: {}", e))?;
            
        if !resp.status().is_success() {
             return Err(format!("Sessions API returned error: {}", resp.status()));
        }
        
        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
            
        let sessions = json.as_array().ok_or("Sessions response is not an array")?;
        
        let mut target_item = None;
        let mut target_ms_id = self.media_source_id.clone().unwrap_or_default();
        
        println!("[MediaServer] Found {} active sessions", sessions.len());

        for session in sessions {
             let now_playing = match session.get("NowPlayingItem") {
                 Some(i) => i,
                 None => continue
             };
             
             let play_state = match session.get("PlayState") {
                 Some(p) => p,
                 None => continue
             };
             
             let ms_id = play_state["MediaSourceId"].as_str().unwrap_or("");
             let item_id = now_playing["Id"].as_str().unwrap_or("");
             
             // Check match
             let match_by_ms_id = !target_ms_id.is_empty() && ms_id == target_ms_id;
             let match_by_item_id = item_id == self.item_id;
             
             if match_by_ms_id || match_by_item_id {
                 println!("[MediaServer] Found matching session. Device: {}", session["DeviceId"]);
                 target_item = Some(now_playing);
                 if target_ms_id.is_empty() {
                     target_ms_id = ms_id.to_string();
                 }
                 break;
             }
        }
        
        let item = target_item.ok_or_else(|| {
            format!("Could not find active session for item {} / source {}", self.item_id, target_ms_id)
        })?;
        
        let mut tracks = Vec::new();
        if let Some(streams) = item["MediaStreams"].as_array() {
             for stream in streams {
                 let is_text = stream["IsTextSubtitleStream"].as_bool().unwrap_or(false);
                 let type_str = stream["Type"].as_str().unwrap_or("");
                 
                 if type_str == "Subtitle" || is_text {
                     let index = stream["Index"].as_i64().unwrap_or(-1) as i32;
                     let title = stream["DisplayTitle"].as_str().unwrap_or("Unknown").to_string();
                     let lang = stream["Language"].as_str().unwrap_or("").to_string();
                     let codec = stream["Codec"].as_str().unwrap_or("srt").to_string();
                     let is_default = stream["IsDefault"].as_bool().unwrap_or(false);
                     let is_forced = stream["IsForced"].as_bool().unwrap_or(false);
                     
                     let ext = "srt";
                     
                     // Use the ItemID from the session object to be safe
                     let final_item_id = item["Id"].as_str().unwrap_or(&self.item_id);
                     
                     let url = format!("{}/Videos/{}/{}/Subtitles/{}/Stream.{}?api_key={}", 
                        self.base_url, final_item_id, target_ms_id, index, ext, self.api_key);

                     tracks.push(SubtitleTrack {
                         index,
                         title,
                         language: lang,
                         codec,
                         is_default,
                         is_forced,
                         download_url: url,
                     });
                 }
             }
        }
        
        Ok(tracks)
    }
}
