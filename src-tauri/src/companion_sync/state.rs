//! `paired_devices.json` · the local registry of iPhones the user has
//! paired with this Mac. Holds only non-sensitive metadata · client_id
//! (used as the Keychain account key), friendly name, paired-at, last-seen,
//! last-window-sent. Real secrets live in the macOS Keychain.
//!
//! Schema is versioned so future migrations can be additive. Reads are
//! tolerant · a missing or malformed file becomes an empty registry, never
//! a crash, because losing the registry just means the next phone connect
//! will go through pairing again (the worst-case is a friction prompt, not
//! data loss).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;

const FILENAME: &str = "companion_paired_devices.json";
const VERSION_CURRENT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedDevice {
    pub client_id: String,
    pub client_name: String,
    pub paired_at: String,
    pub last_seen: Option<String>,
    pub last_window_sent_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevices {
    pub version: u32,
    pub devices: Vec<PairedDevice>,
}

impl Default for PairedDevices {
    fn default() -> Self {
        Self {
            version: VERSION_CURRENT,
            devices: Vec::new(),
        }
    }
}

fn registry_path() -> Option<PathBuf> {
    Some(config::app_data_dir()?.join(FILENAME))
}

pub fn load() -> PairedDevices {
    let Some(path) = registry_path() else {
        return PairedDevices::default();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return PairedDevices::default();
    };
    serde_json::from_slice::<PairedDevices>(&bytes).unwrap_or_default()
}

pub fn save(state: &PairedDevices) -> Result<(), String> {
    let path = registry_path().ok_or_else(|| "no app_data_dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Insert a new pairing or refresh an existing one (re-pair from the same
/// phone overwrites name + paired_at). Caller passes RFC 3339 timestamps.
pub fn upsert(client_id: &str, client_name: &str, now_rfc3339: &str) -> Result<(), String> {
    let mut state = load();
    if let Some(existing) = state.devices.iter_mut().find(|d| d.client_id == client_id) {
        existing.client_name = client_name.to_string();
        existing.paired_at = now_rfc3339.to_string();
        existing.last_seen = Some(now_rfc3339.to_string());
    } else {
        state.devices.push(PairedDevice {
            client_id: client_id.to_string(),
            client_name: client_name.to_string(),
            paired_at: now_rfc3339.to_string(),
            last_seen: Some(now_rfc3339.to_string()),
            last_window_sent_at: None,
        });
    }
    save(&state)
}

pub fn touch_last_seen(client_id: &str, now_rfc3339: &str) -> Result<(), String> {
    let mut state = load();
    if let Some(d) = state.devices.iter_mut().find(|d| d.client_id == client_id) {
        d.last_seen = Some(now_rfc3339.to_string());
    }
    save(&state)
}

pub fn touch_window_sent(client_id: &str, now_rfc3339: &str) -> Result<(), String> {
    let mut state = load();
    if let Some(d) = state.devices.iter_mut().find(|d| d.client_id == client_id) {
        d.last_window_sent_at = Some(now_rfc3339.to_string());
        d.last_seen = Some(now_rfc3339.to_string());
    }
    save(&state)
}

pub fn remove(client_id: &str) -> Result<(), String> {
    let mut state = load();
    state.devices.retain(|d| d.client_id != client_id);
    save(&state)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure-data round-trip · the registry shape must survive JSON without
    /// touching the real app_data_dir.
    #[test]
    fn registry_round_trips_through_json() {
        let orig = PairedDevices {
            version: 1,
            devices: vec![PairedDevice {
                client_id: "c-1".into(),
                client_name: "Neha's iPhone".into(),
                paired_at: "2026-05-20T10:00:00Z".into(),
                last_seen: Some("2026-05-20T10:02:14Z".into()),
                last_window_sent_at: None,
            }],
        };
        let s = serde_json::to_string(&orig).unwrap();
        let back: PairedDevices = serde_json::from_str(&s).unwrap();
        assert_eq!(back.devices, orig.devices);
        assert_eq!(back.version, 1);
    }

    #[test]
    fn malformed_json_falls_back_to_default() {
        let r: Result<PairedDevices, _> = serde_json::from_str("{not json");
        let state = r.unwrap_or_default();
        assert!(state.devices.is_empty());
        assert_eq!(state.version, VERSION_CURRENT);
    }
}
