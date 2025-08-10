#[cfg(windows)]
use std::fs::File;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::{
    io::{
        Read,
        Write,
    },
    sync::{
        atomic::{
            AtomicU32,
            Ordering,
        },
        Mutex,
    },
    time::{
        Duration,
        Instant,
    },
};

use super::types::{
    default_mpv_endpoint,
    ConnectionState,
    MpvCommand,
    MpvResponse,
    PendingRequest,
    MPV_BUFFER_SIZE,
    MPV_DETECTION_INTERVAL_MS,
    MPV_REQUEST_TIMEOUT_SECS,
    MPV_SOCKET_TIMEOUT_MS,
};
use crate::core::errors::YomineError;

pub struct MpvManager {
    state: Mutex<ConnectionState>,
    last_check: Mutex<Option<Instant>>,
    confirmed_timestamps: Mutex<Vec<String>>,
    pending_requests: Mutex<Vec<PendingRequest>>,
    request_counter: AtomicU32,
}

impl Default for MpvManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MpvManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ConnectionState::Disconnected),
            last_check: Mutex::new(None),
            confirmed_timestamps: Mutex::new(Vec::new()),
            pending_requests: Mutex::new(Vec::new()),
            request_counter: AtomicU32::new(1),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.state.lock().map(|state| *state == ConnectionState::Connected).unwrap_or(false)
    }

    pub fn get_confirmed_timestamps(&self) -> Vec<String> {
        self.confirmed_timestamps.lock().map(|timestamps| timestamps.clone()).unwrap_or_default()
    }

    pub fn update(&self) {
        let now = Instant::now();

        {
            let mut last_check = match self.last_check.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };

            let should_check = match *last_check {
                None => true,
                Some(prev) => {
                    now.duration_since(prev) >= Duration::from_millis(MPV_DETECTION_INTERVAL_MS)
                }
            };

            if !should_check {
                return;
            }
            *last_check = Some(now);
        }

        let detected = self.detect_mpv();

        if let Ok(mut state) = self.state.lock() {
            *state =
                if detected { ConnectionState::Connected } else { ConnectionState::Disconnected };
        }

        self.cleanup_old_requests(now);
    }

    pub fn seek_timestamp(&self, seconds: f64, timestamp_str: &str) -> Result<(), YomineError> {
        if !self.is_connected() {
            return Err(YomineError::Custom("MPV is not connected".into()));
        }

        let request_id = self.request_counter.fetch_add(1, Ordering::SeqCst);
        let command = self.create_seek_command(seconds, request_id)?;
        let payload = format!(
            "{}\n",
            serde_json::to_string(&command).map_err(|e| YomineError::Custom(format!(
                "Failed to serialize MPV command: {}",
                e
            )))?
        );

        let mut connection = self.create_connection()?;
        connection
            .write_all(payload.as_bytes())
            .map_err(|e| YomineError::Custom(format!("Failed to write to MPV IPC: {}", e)))?;

        let pending_request = PendingRequest {
            request_id,
            timestamp_str: timestamp_str.to_string(),
            sent_time: Instant::now(),
        };

        if let Ok(mut pending) = self.pending_requests.lock() {
            pending.push(pending_request);
        } else {
            eprintln!("[MPV] Failed to store pending request due to mutex poisoning");
        }

        println!("[MPV] Sent seek command with request_id: {} (pending confirmation)", request_id);

        let mut buf = [0u8; MPV_BUFFER_SIZE];
        match connection.read(&mut buf) {
            Ok(n) if n > 0 => {
                let resp_str = String::from_utf8_lossy(&buf[..n]);

                for line in resp_str.lines() {
                    if let Ok(resp) = serde_json::from_str::<MpvResponse>(line) {
                        if resp.request_id.is_some() {
                            self.handle_response(resp);
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("[MPV] Failed to read response for request_id {}: {}", request_id, e);
            }
        }

        Ok(())
    }

    fn create_seek_command(
        &self,
        seconds: f64,
        request_id: u32,
    ) -> Result<MpvCommand, YomineError> {
        let seconds_value = serde_json::Number::from_f64(seconds)
            .ok_or_else(|| YomineError::Custom("Invalid timestamp value for MPV".into()))?;

        Ok(MpvCommand {
            command: vec![
                serde_json::Value::String("set_property".to_string()),
                serde_json::Value::String("time-pos".to_string()),
                serde_json::Value::Number(seconds_value),
            ],
            request_id,
        })
    }

    fn create_connection(&self) -> Result<Box<dyn ReadWrite>, YomineError> {
        let endpoint = default_mpv_endpoint();

        #[cfg(unix)]
        {
            let stream = UnixStream::connect(endpoint).map_err(|e| {
                YomineError::Custom(format!("Failed to connect to MPV IPC {}: {}", endpoint, e))
            })?;

            let timeout = Duration::from_millis(MPV_SOCKET_TIMEOUT_MS);
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_write_timeout(Some(timeout));
            Ok(Box::new(stream))
        }

        #[cfg(windows)]
        {
            let pipe =
                std::fs::OpenOptions::new().read(true).write(true).open(endpoint).map_err(|e| {
                    YomineError::Custom(format!(
                        "Failed to connect to MPV pipe {}: {}",
                        endpoint, e
                    ))
                })?;
            Ok(Box::new(pipe))
        }
    }

    fn handle_response(&self, resp: MpvResponse) {
        if let Some(resp_id) = resp.request_id {
            let mut pending = match self.pending_requests.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };

            if let Some(pos) = pending.iter().position(|req| req.request_id == resp_id) {
                let request = pending.remove(pos);

                if resp.error == "success" {
                    self.push_confirmed(&request.timestamp_str);
                    println!(
                        "[MPV] Confirmed seek for timestamp: {} (request_id: {})",
                        request.timestamp_str, resp_id
                    );
                } else {
                    eprintln!(
                        "[MPV] Seek failed for timestamp: {} (request_id: {}, error: {})",
                        request.timestamp_str, resp_id, resp.error
                    );
                }
            }
        }
    }

    fn push_confirmed(&self, timestamp_str: &str) {
        let mut list = match self.confirmed_timestamps.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        list.push(timestamp_str.to_string());
    }

    fn cleanup_old_requests(&self, now: Instant) {
        let mut pending = match self.pending_requests.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let timeout = Duration::from_secs(MPV_REQUEST_TIMEOUT_SECS);

        pending.retain(|req| {
            let elapsed = now.duration_since(req.sent_time);
            if elapsed > timeout {
                eprintln!(
                    "[MPV] Request timeout for timestamp: {} (request_id: {})",
                    req.timestamp_str, req.request_id
                );
                false
            } else {
                true
            }
        });
    }

    fn detect_mpv(&self) -> bool {
        let endpoint = default_mpv_endpoint();

        #[cfg(unix)]
        {
            use std::path::Path;
            Path::new(endpoint).exists() && UnixStream::connect(endpoint).is_ok()
        }

        #[cfg(windows)]
        {
            std::fs::OpenOptions::new().read(true).write(true).open(endpoint).is_ok()
        }
    }
}

trait ReadWrite: Read + Write {}

#[cfg(unix)]
impl ReadWrite for UnixStream {}

#[cfg(windows)]
impl ReadWrite for File {}
