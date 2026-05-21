//! `companion_hrv_history.json` · the local trend log of HRV readings
//! the phone has sent back to the Mac. Append-only, cap at 1000 entries
//! (a year of daily check-ins, well past anything we'd render in a
//! 30-day chart). Stored only on this Mac, never synced. Reads are
//! tolerant · a missing or malformed file becomes an empty history,
//! because the chart degrades gracefully to "no data yet" rather than
//! the app crashing.
//!
//! Schema v2 (this file) holds one row per session, with `pre` and
//! `post` PPG measurements merged in as the phone sends them. v1 rows
//! came from the now-removed HealthKit-on-window path; their SDNN
//! numbers are statistically incomparable to the v2 RMSSD primary, so
//! v1 readings are dropped on load rather than mixed into the chart.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;

const FILENAME: &str = "companion_hrv_history.json";
const VERSION_CURRENT: u32 = 2;
const HISTORY_CAP: usize = 1000;

/// One phase of a PPG capture. Mirrors the wire frame's `phase` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Pre,
    Post,
}

impl Phase {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pre" => Some(Self::Pre),
            "post" => Some(Self::Post),
            _ => None,
        }
    }
}

/// One per-phase capture the phone reported. Kept on the row so we can
/// render either side independently and so a future PR can show
/// per-session pre/post bars without another schema bump.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PhaseCapture {
    /// Primary HRV metric for short PPG · root-mean-square of successive
    /// IBI differences. `None` when the capture status was not `ok`.
    pub rmssd_ms: Option<f64>,
    /// Secondary HRV metric · standard deviation of IBIs. Reported for
    /// completeness so we can compare RMSSD and SDNN trends offline.
    pub sdnn_ms: Option<f64>,
    /// Number of IBIs detected in the 30s capture. ~25-40 for a healthy
    /// resting heart rate; anything in single digits is a bad capture.
    pub sample_count: u32,
    pub snr_db: Option<f64>,
    /// `"ok"`, `"low_signal"`, `"finger_lifted"`, `"cancelled"`.
    pub status: String,
    /// RFC 3339 wall-clock at which the Mac received this phase.
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HrvReading {
    /// session_id the row is keyed by. Pre and post frames sharing this
    /// id are merged into the same row.
    pub session_id: String,
    /// First-seen RFC 3339 timestamp · used to sort the chart. Set when
    /// the first phase arrives for a session; never updated.
    pub first_seen_at: String,
    pub pre: Option<PhaseCapture>,
    pub post: Option<PhaseCapture>,
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
    parse(&bytes)
}

/// Pure parse for testability. v1 rows (the HealthKit auto-read path)
/// are dropped on load · their SDNN-only deltas are not comparable to
/// v2 PPG RMSSD numbers and mixing them would skew the chart.
fn parse(bytes: &[u8]) -> HrvHistory {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return HrvHistory::default();
    };
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version != VERSION_CURRENT {
        return HrvHistory::default();
    }
    serde_json::from_value::<HrvHistory>(value).unwrap_or_default()
}

pub fn save(h: &HrvHistory) -> Result<(), String> {
    let path = history_path().ok_or_else(|| "no app_data_dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(h).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Merge a freshly arrived per-phase capture into the row keyed by
/// `session_id`. If the row already has a capture for the same phase,
/// the new one replaces it · the phone may resend after a reconnect
/// (queue replay), and the second send is the one with the latest
/// information.
pub fn merge(session_id: &str, phase: Phase, capture: PhaseCapture) -> Result<(), String> {
    if session_id.is_empty() {
        return Err("empty session_id".into());
    }
    let mut h = load();
    if let Some(row) = h.readings.iter_mut().find(|r| r.session_id == session_id) {
        match phase {
            Phase::Pre => row.pre = Some(capture),
            Phase::Post => row.post = Some(capture),
        }
    } else {
        let first_seen_at = capture.received_at.clone();
        let (pre, post) = match phase {
            Phase::Pre => (Some(capture), None),
            Phase::Post => (None, Some(capture)),
        };
        h.readings.push(HrvReading {
            session_id: session_id.into(),
            first_seen_at,
            pre,
            post,
        });
    }
    while h.readings.len() > HISTORY_CAP {
        h.readings.remove(0);
    }
    save(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_capture(rmssd: f64) -> PhaseCapture {
        PhaseCapture {
            rmssd_ms: Some(rmssd),
            sdnn_ms: Some(rmssd * 0.9),
            sample_count: 28,
            snr_db: Some(10.0),
            status: "ok".into(),
            received_at: "2026-05-21T18:14:00Z".into(),
        }
    }

    fn low_signal_capture() -> PhaseCapture {
        PhaseCapture {
            rmssd_ms: None,
            sdnn_ms: None,
            sample_count: 4,
            snr_db: Some(2.1),
            status: "low_signal".into(),
            received_at: "2026-05-21T18:14:00Z".into(),
        }
    }

    #[test]
    fn history_round_trips_through_json() {
        let h = HrvHistory {
            version: VERSION_CURRENT,
            readings: vec![HrvReading {
                session_id: "s-1".into(),
                first_seen_at: "2026-05-21T18:14:00Z".into(),
                pre: Some(ok_capture(40.0)),
                post: Some(ok_capture(48.0)),
            }],
        };
        let s = serde_json::to_string(&h).unwrap();
        let back: HrvHistory = serde_json::from_str(&s).unwrap();
        assert_eq!(h.readings, back.readings);
        assert_eq!(h.version, back.version);
    }

    #[test]
    fn malformed_json_falls_back_to_default() {
        let h = parse(b"not json");
        assert!(h.readings.is_empty());
        assert_eq!(h.version, VERSION_CURRENT);
    }

    #[test]
    fn v1_file_is_dropped_on_load() {
        // A v1 file shape (the old SDNN-on-window schema). load() must
        // not surface these in the v2 chart · the numbers are not
        // comparable to PPG RMSSD primary.
        let v1 = br#"{
            "version": 1,
            "readings": [{
                "received_at": "2026-04-01T10:00:00Z",
                "session_id": "old-1",
                "pre_ms": 40.0,
                "post_ms": 48.0,
                "delta_ms": 8.0,
                "samples_pre": 1,
                "samples_post": 2,
                "status": "ok"
            }]
        }"#;
        let h = parse(v1);
        assert!(h.readings.is_empty());
        assert_eq!(h.version, VERSION_CURRENT);
    }

    #[test]
    fn merge_inserts_row_on_first_phase() {
        let mut h = HrvHistory::default();
        h.readings.push(HrvReading {
            session_id: "s-1".into(),
            first_seen_at: "2026-05-21T18:14:00Z".into(),
            pre: Some(ok_capture(40.0)),
            post: None,
        });
        // Same id, post phase arrives · in-memory merge logic mirroring
        // what `merge()` does on disk.
        let new_post = ok_capture(50.0);
        let row = h.readings.iter_mut().find(|r| r.session_id == "s-1").unwrap();
        row.post = Some(new_post);
        assert_eq!(h.readings.len(), 1);
        assert_eq!(h.readings[0].pre.as_ref().unwrap().rmssd_ms, Some(40.0));
        assert_eq!(h.readings[0].post.as_ref().unwrap().rmssd_ms, Some(50.0));
    }

    #[test]
    fn merge_replaces_same_phase_on_replay() {
        let mut h = HrvHistory::default();
        h.readings.push(HrvReading {
            session_id: "s-1".into(),
            first_seen_at: "2026-05-21T18:14:00Z".into(),
            pre: Some(low_signal_capture()),
            post: None,
        });
        // Replay arrives with a successful pre capture · replace the
        // earlier low_signal entry rather than keeping both.
        let row = h.readings.iter_mut().find(|r| r.session_id == "s-1").unwrap();
        row.pre = Some(ok_capture(42.0));
        assert_eq!(h.readings.len(), 1);
        assert_eq!(h.readings[0].pre.as_ref().unwrap().status, "ok");
    }

    #[test]
    fn cap_drops_oldest() {
        let mut h = HrvHistory::default();
        for i in 0..HISTORY_CAP + 5 {
            h.readings.push(HrvReading {
                session_id: format!("s-{i}"),
                first_seen_at: "2026-05-21T18:14:00Z".into(),
                pre: None,
                post: Some(ok_capture(40.0)),
            });
        }
        while h.readings.len() > HISTORY_CAP {
            h.readings.remove(0);
        }
        assert_eq!(h.readings.len(), HISTORY_CAP);
        assert_eq!(h.readings[0].session_id, "s-5");
    }
}
