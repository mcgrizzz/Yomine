//! Tsunagi's own API (`/v1`), next to the AnkiConnect shim on the same port.

use std::{
    net::{
        Ipv4Addr,
        Ipv6Addr,
        SocketAddr,
    },
    sync::LazyLock,
};

use reqwest::Client;
use serde::Deserialize;

use super::{
    client::{
        Anki,
        PROBE_TIMEOUT,
    },
    connection::base_url,
};

pub(super) const DEFAULT_PORT: u16 = 7777;

/// Keeps its connections open, unlike AnkiConnect's client: Tsunagi's server reuses them,
/// and a new one per request can cost more than the request.
static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    let loopback =
        [SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), SocketAddr::from((Ipv6Addr::LOCALHOST, 0))];
    Client::builder()
        // Tsunagi binds IPv4 by default; trying ::1 first stalls each request on Windows.
        .resolve_to_addrs("localhost", &loopback)
        .build()
        .expect("failed to create Tsunagi HTTP client")
});

#[derive(Deserialize)]
struct Health {
    server: String,
    versions: Versions,
}

#[derive(Deserialize)]
struct Versions {
    api: String,
    addon: String,
}

/// Tsunagi's release when its `v1` API answers here. The health endpoint is public, so
/// this doesn't check the key.
pub(super) async fn version(anki: &Anki) -> Option<String> {
    let health: Health = CLIENT
        .get(format!("{}/v1/health", base_url(&anki.connection)))
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    (health.server == "tsunagi" && health.versions.api == "v1").then_some(health.versions.addon)
}
