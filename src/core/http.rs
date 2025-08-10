use std::{
    fs::File,
    io::{
        BufWriter,
        Read,
        Write,
    },
    path::Path,
    time::Duration,
};

use reqwest::{
    blocking::{
        Client,
        Response,
    },
    header::{
        ACCEPT_ENCODING,
        USER_AGENT,
    },
};

use crate::core::YomineError;

pub fn http_client() -> Result<Client, YomineError> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| YomineError::Custom(format!("HTTP client build failed: {e}")))
}

pub fn download_with_progress(
    client: &Client,
    url: &str,
    path: &Path,
    message_callback: Option<&Box<dyn Fn(String) + Send>>,
) -> Result<(), YomineError> {
    let mut attempts: usize = 0;
    loop {
        attempts += 1;

        let resp = client
            .get(url)
            .header(USER_AGENT, "yomine/1.0 (+reqwest)")
            .header(ACCEPT_ENCODING, "identity")
            .send();

        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                if attempts < 3 {
                    std::thread::sleep(Duration::from_secs(2 * attempts as u64));
                    continue;
                }
                return Err(YomineError::Custom(format!("Failed HTTP GET {}: {}", url, e)));
            }
        };

        ensure_success(&resp)?;

        let mut writer = BufWriter::new(File::create(path).map_err(|e| {
            YomineError::Custom(format!("Create download file {:?} failed: {}", path, e))
        })?);

        // Get content length for progress reporting
        let total_size = resp.content_length();
        let mut downloaded = 0u64;

        // Read in chunks and report progress
        let mut buffer = [0; 8192]; // 8KB chunks
        loop {
            match resp.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(bytes_read) => {
                    writer.write_all(&buffer[..bytes_read]).map_err(|e| {
                        YomineError::Custom(format!("Failed to write to file: {}", e))
                    })?;

                    downloaded += bytes_read as u64;

                    // Report progress if callback is provided
                    if let Some(ref callback) = message_callback {
                        let progress_msg = if let Some(total_bytes) = total_size {
                            format!(
                                "Downloading Tokenizer Model: {:.1}MB/{:.1}MB ({:.1}%)",
                                downloaded as f64 / 1_048_576.0,
                                total_bytes as f64 / 1_048_576.0,
                                (downloaded as f64 / total_bytes as f64) * 100.0
                            )
                        } else {
                            format!(
                                "Downloading Tokenizer Model: {:.1}MB",
                                downloaded as f64 / 1_048_576.0
                            )
                        };
                        callback(progress_msg);
                    }
                }
                Err(e) => {
                    if attempts < 3 {
                        std::thread::sleep(Duration::from_secs(2 * attempts as u64));
                        break;
                    }
                    return Err(YomineError::Custom(format!("Failed to read response: {}", e)));
                }
            }
        }

        if downloaded > 0 {
            writer
                .flush()
                .map_err(|e| YomineError::Custom(format!("Failed to flush file: {}", e)))?;
            return Ok(());
        } else if attempts < 3 {
            std::thread::sleep(Duration::from_secs(2 * attempts as u64));
            continue;
        } else {
            return Err(YomineError::Custom("Failed to download any data".to_string()));
        }
    }
}

fn ensure_success(resp: &Response) -> Result<(), YomineError> {
    if !resp.status().is_success() {
        return Err(YomineError::Custom(format!(
            "HTTP error {} from {}",
            resp.status(),
            resp.url()
        )));
    }
    Ok(())
}
