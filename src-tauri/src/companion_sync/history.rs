//! `companion_hrv_history.json` · the local trend log of HRV readings
//! the phone has sent back to the Mac. Append-only, cap at 1000 entries
//! (a year of daily check-ins, well past anything we'd render in a
//! 30-day chart). Stored only on this Mac, never synced. Reads are
//! tolerant · a missing or malformed file becomes an empty history,
//! because the chart degrades gracefully to "no data yet" rather than
//! the app crashing.
//!
//! Schema is versioned for future migrations (e.g. adding `kind` to
//! distinguish session deltas vs explicit check-ins vs morning baseline).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;

const FILENAME: &str = "companion_hrv_history.json";
const VERSION_CURRENT: u32 = 1;
const HISTORY_CAP: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HrvReading {
    /// Wall-clock RFC 3339 timestamp at which the Mac received the frame.
    /// We use the receive time (not the session start) so the chart
    /// renders by "when did this data arrive" regardless of whether the
    /// reading was from a session window, a check-in, or a back-fill.
    pub received_at: String,
    /// Optional session_id the reading is associated with. Empty when
    /// the phone sends a stand-alone check-in (PR4-iOS will add this).
    pub session_id: String,
    pub pre_ms: Option<f64>,
    pub post_ms: Option<f64>,
    pub delta_ms: Option<f64>,
    pub samples_pre: u32,
    pub samples_post: u32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrvHistory {
    pub version: u32,
    pub readings: Vec<HrvReading>,
}

impl Default for HrvHistory {
    fn default() -> Self {
        Self {
            version: VERSION_CURRENT,
            readings: Vec::new(),
        }
    }
}

fn history_path() -> Option<PathBuf> {
    Some(config::app_data_dir()?.join(FILENAME))
}

pub fn load() -> HrvHistory {
    let Some(path) = history_path() else {
        return HrvHistory::default();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return HrvHistory::default();
    };
    serde_json::from_slice::<HrvHistory>(&bytes).unwrap_or_default()
}

pub fn save(h: &HrvHistory) -> Result<(), String> {
    let path = history_path().ok_or_else(|| "no app_data_dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(h).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Append a reading. If a reading with the same `session_id` already
/// exists (e.g. phone re-sent after reconnect with queue replay), the
/// existing entry is replaced rather than duplicated, keeping the
/// chart honest about how many actual check-ins occurred.
pub fn record(reading: HrvReading) -> Result<(), String> {
    let mut h = load();
    if !reading.session_id.is_empty() {
        h.readings.retain(|r| r.session_id != reading.session_id);
    }
    h.readings.push(reading);
    while h.readings.len() > HISTORY_CAP {
        h.readings.remove(0);
    }
    save(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, post: Option<f64>) -> HrvReading {
        HrvReading {
            received_at: "2026-05-21T18:14:00Z".into(),
            session_id: id.into(),
            pre_ms: None,
            post_ms: post,
            delta_ms: None,
            samples_pre: 0,
            samples_post: if post.is_some() { 3 } else { 0 },
            status: if post.is_some() { "ok".into() } else { "no_data".into() },
        }
    }

    #[test]
    fn history_round_trips_through_json() {
        let h = HrvHistory {
            version: 1,
            readings: vec![sample("s-1", Some(42.5)), sample("s-2", None)],
        };
        let s = serde_json::to_string(&h).unwrap();
        let back: HrvHistory = serde_json::from_str(&s).unwrap();
        assert_eq!(h.readings, back.readings);
        assert_eq!(h.version, back.version);
    }

    #[test]
    fn malformed_json_falls_back_to_default() {
        let r: Result<HrvHistory, _> = serde_json::from_str("not json");
        let h = r.unwrap_or_default();
        assert!(h.readings.is_empty());
        assert_eq!(h.version, VERSION_CURRENT);
    }

    #[test]
    fn dedupe_replaces_existing_session_in_memory() {
        let mut h = HrvHistory::default();
        h.readings.push(sample("s-1", Some(40.0)));
        // Simulate the dedupe step in `record`.
        let new = sample("s-1", Some(45.0));
        h.readings.retain(|r| r.session_id != new.session_id);
        h.readings.push(new);
        assert_eq!(h.readings.len(), 1);
        assert_eq!(h.readings[0].post_ms, Some(45.0));
    }

    #[test]
    fn cap_drops_oldest() {
        let mut h = HrvHistory::default();
        for i in 0..HISTORY_CAP + 5 {
            h.readings.push(sample(&format!("s-{i}"), Some(40.0)));
        }
        while h.readings.len() > HISTORY_CAP {
            h.readings.remove(0);
        }
        assert_eq!(h.readings.len(), HISTORY_CAP);
        assert_eq!(h.readings[0].session_id, "s-5");
    }
}
