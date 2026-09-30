use std::{
    num::NonZeroU16,
    sync::{
        LazyLock,
        RwLock,
    },
};

use super::{
    client::Backend,
    tsunagi,
};
use crate::core::settings::AnkiConnectionSettings;

static CONNECTION: LazyLock<RwLock<AnkiConnectionSettings>> =
    LazyLock::new(|| RwLock::new(AnkiConnectionSettings::default()));

/// The add-on the configured connection last answered as; `None` until it answers.
static BACKEND: RwLock<Option<Backend>> = RwLock::new(None);

pub fn configure(settings: AnkiConnectionSettings) {
    *CONNECTION.write().unwrap() = settings;
    *BACKEND.write().unwrap() = None;
}

pub(super) fn active() -> AnkiConnectionSettings {
    CONNECTION.read().unwrap().clone()
}

pub(super) fn backend() -> Option<Backend> {
    BACKEND.read().unwrap().clone()
}

/// Records what `connection` answered as, unless the configured connection changed since.
pub(super) fn set_backend(connection: &AnkiConnectionSettings, backend: Option<Backend>) {
    let configured = CONNECTION.read().unwrap();
    if *configured == *connection {
        *BACKEND.write().unwrap() = backend;
    }
}

pub(super) fn base_url(connection: &AnkiConnectionSettings) -> String {
    let host = connection.host.trim();
    let host = if host.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    format!("http://{host}:{}", connection.port)
}

/// While `current` is still AnkiConnect's default, a Tsunagi answering on its own
/// default port.
pub async fn default_tsunagi(current: &AnkiConnectionSettings) -> Option<AnkiConnectionSettings> {
    let default = AnkiConnectionSettings::default();
    if *current != default {
        return None;
    }
    let candidate =
        AnkiConnectionSettings { port: NonZeroU16::new(tsunagi::DEFAULT_PORT).unwrap(), ..default };
    match super::probe(candidate.clone()).detect().await {
        Ok(Backend::Tsunagi { .. }) => Some(candidate),
        _ => None,
    }
}
