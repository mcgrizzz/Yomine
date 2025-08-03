use std::{
    fs::File,
    io::{
        BufWriter,
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

pub fn download_to_file(client: &Client, url: &str, path: &Path) -> Result<(), YomineError> {
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

        let res = resp.copy_to(&mut writer);
        match res {
            Ok(n) if n > 0 => {
                writer.flush().ok();
                return Ok(());
            }
            Ok(_) | Err(_) => {
                if attempts < 3 {
                    std::thread::sleep(Duration::from_secs(2 * attempts as u64));
                    continue;
                }
                return Err(YomineError::Custom(
                    "Failed to copy response body to file".to_string(),
                ));
            }
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
