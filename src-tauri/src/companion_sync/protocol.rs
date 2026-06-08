//! Wire protocol v4 between the Niyora Mac app and the Niyora Companion
//! iPhone app. Every app message is one JSON object carrying a `type`
//! discriminator. Unknown types are a fatal error and the connection is
//! dropped, on the principle that a misaligned client is more dangerous
//! than a disconnect.
//!
//! As of v4 the channel is wrapped in a Noise XX handshake (see
//! `noise.rs`): app messages are sealed Noise transport frames, not
//! plaintext NDJSON. The handshake replaces the old QR-secret + HMAC
//! challenge entirely.
//!
//! Lifecycle from the phone's perspective:
//!
//! ```text
//!   [Noise XX handshake · 3 messages]
//!   [both derive the SAS from the handshake hash]
//!   →  identify             {protocol, client_id, client_name}
//!   ←  hello                {protocol, server_id, server_name}
//!   ←  authed   or   ← auth_failed (then close)
//!   ←  status_update        {soul_tier, completed_sessions, current_tier, total_session_count, mac_native_completed, mac_native_total}
//!   ←  soul_state           {day_label, index, source, ts}
//!   ←  request_measurement  {session_id, phase, technique_name}   (zero or more per session)
//!   →  session_recorded     {technique_name, duration_sec, completed, recorded_at}
//! ```
//!
//! Trust: a known phone (its static key matches the one the Mac stored at
//! pairing) is reconnected silently. An unknown phone is paired only when
//! the Mac user clicks Allow during an open pairing window, after both
//! confirm the SAS matches. There is no `pairing_id` and no shared secret.
//!
//! v4 vs v3: the transport is now Noise-sealed; the QR + HMAC auth path
//! (`pairing_id` on identify, `challenge`/`auth` frames) is removed. v3 and
//! earlier companions cannot speak v4 and must update.

use serde::{Deserialize, Serialize};

/// Wire-format protocol version. v4 wraps the channel in Noise XX and drops
/// the QR/HMAC handshake. Bumped on any breaking shape change.
pub const PROTOCOL_VERSION: u32 = 4;

/// Minimum protocol version accepted from companions. v4 is a hard break
/// from the plaintext + HMAC transport · pre-v4 clients cannot handshake.
pub const PROTOCOL_VERSION_MIN: u32 = 4;

fn default_technique_kind() -> String {
    "breathing".to_string()
}

/// A message from the phone to the Mac.
/// `Eq` is not derived because `HrvResult` carries `f64` fields, which do
/// not implement `Eq` (NaN != NaN). All call sites use `assert_eq!` /
/// `PartialEq`, which is enough.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// First sealed frame after the Noise handshake. The phone's identity is
    /// the static key the handshake established; `client_id` is just a stable
    /// handle for the Keychain entry, registry, and queue dedup. There is no
    /// `pairing_id` and no secret.
    Identify {
        protocol: u32,
        client_id: String,
        client_name: String,
    },
    /// A completed (or abandoned) session from the iOS companion.
    /// The Mac persists this to the session log and bumps Soul tier
    /// progression. v2 companions never send this.
    SessionRecorded {
        technique_name: String,
        /// `"breathing"` or `"mindfulness"`. Defaults to `"breathing"` for
        /// early v3 companion builds that predate this field; later builds
        /// must send the real kind so mindfulness sessions are logged
        /// correctly.
        #[serde(default = "default_technique_kind")]
        technique_kind: String,
        /// How long the session actually ran. Always present.
        duration_sec: u64,
        /// How long the session was meant to run. Distinct from
        /// `duration_sec` when `completed` is false (the user abandoned
        /// early). Optional for forward compat: when absent, the Mac
        /// assumes intended == actual.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intended_duration_sec: Option<u64>,
        completed: bool,
        /// ISO 8601 timestamp of when the session was recorded on the phone.
        recorded_at: String,
    },
    /// One PPG measurement the phone captured for a session. The Mac merges
    /// pre and post frames sharing the same `session_id` into one history
    /// row. Status reports the capture quality honestly · the spec's
    /// "honest gaps" rule means a noisy capture is reported as such, not
    /// papered over with a fabricated number.
    HrvResult {
        session_id: String,
        /// Which side of the breathing session this capture covers.
        /// `"pre"` or `"post"`.
        phase: String,
        /// Root-mean-square of successive differences, the standard HRV
        /// metric for short (~30s) PPG recordings. Primary signal.
        #[serde(skip_serializing_if = "Option::is_none")]
        rmssd_ms: Option<f64>,
        /// Standard deviation of inter-beat intervals, secondary signal.
        /// Reported alongside RMSSD so future analysis can compare the two
        /// without re-running captures.
        #[serde(skip_serializing_if = "Option::is_none")]
        sdnn_ms: Option<f64>,
        /// Number of inter-beat intervals detected in the capture.
        sample_count: u32,
        /// Estimated signal-to-noise ratio in decibels. Used by the phone
        /// to decide whether to mark a capture as `low_signal`. Forwarded
        /// to the Mac for future thresholds without another wire change.
        #[serde(skip_serializing_if = "Option::is_none")]
        snr_db: Option<f64>,
        /// `"ok"` · usable RMSSD + SDNN present.
        /// `"low_signal"` · capture finished but SNR was below threshold.
        /// `"finger_lifted"` · capture aborted because the lens lost
        /// contact part-way through.
        /// `"cancelled"` · the user dismissed the sheet before the 30s
        /// elapsed.
        status: String,
    },
    /// Phone foreground/background ping for the active-device rule. Sent by
    /// the iOS app when it becomes active or inactive so the Mac can stay
    /// silent while the phone is in use. Carries no HRV or session data.
    PhoneActive { active: bool, ts: String },
}

/// A message from the Mac to the phone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Sent immediately after a successful `identify` so the phone knows
    /// which Mac it is talking to and can store the secret keyed by
    /// `server_id` for future reconnects.
    Hello {
        protocol: u32,
        server_id: String,
        server_name: String,
    },
    /// Pairing approved (fresh) or recognised (reconnect). App frames may
    /// now flow.
    Authed,
    /// Auth failed. Connection will close after this frame.
    AuthFailed { reason: String },
    /// Mac's current Soul tier and session count, sent after auth and
    /// after each session_recorded so paired mode can display Mac-side
    /// state. v3+ only.
    ///
    /// `soul_tier` and `completed_sessions` are the v3 field names retained
    /// for backward compat with paired-v3 iOS builds. `current_tier` and
    /// `total_session_count` are the v4-companion field names added additively;
    /// old v3 companions ignore them. `total_session_count` counts every
    /// session entry (completed + abandoned), while `completed_sessions`
    /// counts only sessions where `completed == true`.
    StatusUpdate {
        soul_tier: String,
        completed_sessions: u32,
        current_tier: String,
        total_session_count: u32,
        /// Mac-native session counts, excluding sessions the Mac received from the
        /// companion phone. The phone uses these for the "Your Mac" box so that
        /// phone-native + mac-native does not double-count synced sessions.
        /// Added in v4.1; absent on connections from older build stubs (treated as 0).
        #[serde(default)]
        mac_native_completed: u32,
        #[serde(default)]
        mac_native_total: u32,
    },
    /// Tells the phone to start a 30s PPG capture for one side of a
    /// session. The Mac emits these only when the user taps "Measure
    /// stress" on the pre-session info screen or the post-session mood
    /// screen · they are never produced automatically. Idempotent on
    /// `(session_id, phase)`: the phone dedupes if the same request
    /// arrives twice across a reconnect (queue replay).
    RequestMeasurement {
        session_id: String,
        /// `"pre"` or `"post"`. The Mac does not assume one comes before
        /// the other on the wire · queue replay can reorder them.
        phase: String,
        technique_name: String,
    },
    /// Mac's current situational soul-state. Sent after auth (v3+) and
    /// again whenever the day label changes. Only the derived label and
    /// index cross the wire, never the raw collector inputs (screen time,
    /// keystrokes, meetings).
    SoulState {
        /// One of `"calm"`, `"normal"`, `"dense"`, `"heavy"`.
        day_label: String,
        /// Niyora Index 0-100. Higher = calmer day.
        index: u8,
        /// Always `"mac"` in v1. The phone has no passive stress sensor yet.
        #[serde(default)]
        source: String,
        /// ISO 8601 timestamp of when the Mac sent this reading. The phone uses
        /// this to apply a freshness window and hide stale readings.
        #[serde(default)]
        ts: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_serializes_with_type_tag() {
        let m = ClientMessage::Identify {
            protocol: PROTOCOL_VERSION,
            client_id: "c-1".into(),
            client_name: "Neha's iPhone".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "identify");
        assert_eq!(v["client_id"], "c-1");
        assert_eq!(v["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn identify_carries_no_pairing_id() {
        // v4 dropped the QR pairing_id; identity is the Noise static key.
        let m = ClientMessage::Identify {
            protocol: PROTOCOL_VERSION,
            client_id: "c-1".into(),
            client_name: "Neha's iPhone".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("pairing_id").is_none());
    }

    #[test]
    fn server_messages_round_trip() {
        let cases = [
            ServerMessage::Hello {
                protocol: PROTOCOL_VERSION,
                server_id: "s-1".into(),
                server_name: "Neha's MacBook".into(),
            },
            ServerMessage::Authed,
            ServerMessage::AuthFailed {
                reason: "bad hmac".into(),
            },
            ServerMessage::StatusUpdate {
                soul_tier: "glow".into(),
                completed_sessions: 7,
                current_tier: "glow".into(),
                total_session_count: 9,
                mac_native_completed: 5,
                mac_native_total: 7,
            },
            ServerMessage::SoulState {
                day_label: "dense".into(),
                index: 42,
                source: "mac".into(),
                ts: "2026-06-08T12:00:00Z".into(),
            },
            ServerMessage::RequestMeasurement {
                session_id: "sess-1".into(),
                phase: "pre".into(),
                technique_name: "Box Breathing".into(),
            },
            ServerMessage::RequestMeasurement {
                session_id: "sess-1".into(),
                phase: "post".into(),
                technique_name: "Box Breathing".into(),
            },
        ];
        for orig in cases {
            let s = serde_json::to_string(&orig).unwrap();
            let back: ServerMessage = serde_json::from_str(&s).unwrap();
            assert_eq!(orig, back);
        }
    }

    #[test]
    fn unknown_type_fails_decode() {
        let raw = r#"{"type":"surprise","value":42}"#;
        let r: Result<ServerMessage, _> = serde_json::from_str(raw);
        assert!(r.is_err(), "unknown types must not silently decode");
    }

    #[test]
    fn request_measurement_uses_snake_case_tag() {
        let m = ServerMessage::RequestMeasurement {
            session_id: "sess-1".into(),
            phase: "pre".into(),
            technique_name: "Box Breathing".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "request_measurement");
        assert_eq!(v["session_id"], "sess-1");
        assert_eq!(v["phase"], "pre");
        assert_eq!(v["technique_name"], "Box Breathing");
    }

    #[test]
    fn hrv_result_serializes_with_optional_fields() {
        let m = ClientMessage::HrvResult {
            session_id: "sess-1".into(),
            phase: "post".into(),
            rmssd_ms: Some(48.0),
            sdnn_ms: Some(42.5),
            sample_count: 28,
            snr_db: Some(11.2),
            status: "ok".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "hrv_result");
        assert_eq!(v["session_id"], "sess-1");
        assert_eq!(v["phase"], "post");
        assert_eq!(v["rmssd_ms"], 48.0);
        assert_eq!(v["sdnn_ms"], 42.5);
        assert_eq!(v["sample_count"], 28);
        assert_eq!(v["snr_db"], 11.2);
        assert_eq!(v["status"], "ok");
    }

    #[test]
    fn hrv_result_low_signal_omits_metric_fields() {
        let m = ClientMessage::HrvResult {
            session_id: "sess-2".into(),
            phase: "pre".into(),
            rmssd_ms: None,
            sdnn_ms: None,
            sample_count: 0,
            snr_db: Some(3.1),
            status: "low_signal".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("rmssd_ms").is_none());
        assert!(v.get("sdnn_ms").is_none());
        assert_eq!(v["sample_count"], 0);
        assert_eq!(v["status"], "low_signal");
    }

    #[test]
    fn hrv_result_round_trips_through_decode() {
        let m = ClientMessage::HrvResult {
            session_id: "sess-3".into(),
            phase: "post".into(),
            rmssd_ms: Some(45.0),
            sdnn_ms: Some(40.0),
            sample_count: 26,
            snr_db: Some(9.4),
            status: "ok".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: ClientMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn session_recorded_serializes_with_type_tag() {
        let m = ClientMessage::SessionRecorded {
            technique_name: "Ujjayi".into(),
            technique_kind: "breathing".into(),
            duration_sec: 64,
            intended_duration_sec: Some(64),
            completed: true,
            recorded_at: "2026-05-24T12:00:00Z".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "session_recorded");
        assert_eq!(v["technique_name"], "Ujjayi");
        assert_eq!(v["technique_kind"], "breathing");
        assert_eq!(v["duration_sec"], 64);
        assert_eq!(v["intended_duration_sec"], 64);
        assert_eq!(v["completed"], true);
        assert_eq!(v["recorded_at"], "2026-05-24T12:00:00Z");
    }

    #[test]
    fn session_recorded_round_trips() {
        let m = ClientMessage::SessionRecorded {
            technique_name: "Trataka".into(),
            technique_kind: "mindfulness".into(),
            duration_sec: 8,
            intended_duration_sec: Some(60),
            completed: false,
            recorded_at: "2026-05-24T12:30:00Z".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: ClientMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn session_recorded_back_compat_defaults() {
        // Early v3 companion builds may not yet send technique_kind or
        // intended_duration_sec. The Mac must still deserialize their
        // payload, defaulting kind to "breathing" and treating intended
        // as absent.
        let raw = r#"{"type":"session_recorded","technique_name":"Box","duration_sec":60,"completed":true,"recorded_at":"2026-05-24T12:00:00Z"}"#;
        let back: ClientMessage = serde_json::from_str(raw).unwrap();
        match back {
            ClientMessage::SessionRecorded { technique_kind, intended_duration_sec, .. } => {
                assert_eq!(technique_kind, "breathing");
                assert_eq!(intended_duration_sec, None);
            }
            other => panic!("expected SessionRecorded, got {other:?}"),
        }
    }

    #[test]
    fn status_update_serializes_with_type_tag() {
        let m = ServerMessage::StatusUpdate {
            soul_tier: "radiance".into(),
            completed_sessions: 42,
            current_tier: "radiance".into(),
            total_session_count: 45,
            mac_native_completed: 38,
            mac_native_total: 41,
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "status_update");
        assert_eq!(v["soul_tier"], "radiance");
        assert_eq!(v["completed_sessions"], 42);
        assert_eq!(v["mac_native_completed"], 38);
        assert_eq!(v["mac_native_total"], 41);
    }

    #[test]
    fn identify_response_carries_current_tier_and_total_session_count() {
        // Asserts that the status_update sent after an iOS-style identify+auth
        // carries the v4-companion fields alongside the legacy v3 fields.
        // total_session_count counts all entries (completed + abandoned);
        // completed_sessions counts only sessions where completed == true.
        let m = ServerMessage::StatusUpdate {
            soul_tier: "shine".into(),
            completed_sessions: 17,
            current_tier: "shine".into(),
            total_session_count: 20,
            mac_native_completed: 14,
            mac_native_total: 17,
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "status_update");
        assert_eq!(v["current_tier"], "shine");
        assert_eq!(v["total_session_count"], 20);
        // Legacy fields must still be present for v3 companions.
        assert_eq!(v["soul_tier"], "shine");
        assert_eq!(v["completed_sessions"], 17);
        // Native-only counts for the phone's "Your Mac" box.
        assert_eq!(v["mac_native_completed"], 14);
        assert_eq!(v["mac_native_total"], 17);
    }

    #[test]
    fn soul_state_serializes_with_snake_case_tag() {
        let m = ServerMessage::SoulState {
            day_label: "dense".into(),
            index: 42,
            source: "mac".into(),
            ts: "2026-06-08T12:00:00Z".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "soul_state");
        assert_eq!(v["day_label"], "dense");
        assert_eq!(v["index"], 42);
        assert_eq!(v["source"], "mac");
        assert!(v["ts"].as_str().unwrap().starts_with("2026-"));
    }

    #[test]
    fn soul_state_round_trips() {
        let m = ServerMessage::SoulState {
            day_label: "heavy".into(),
            index: 15,
            source: "mac".into(),
            ts: "2026-06-08T09:30:00Z".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: ServerMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn soul_state_ts_absent_deserializes_to_default() {
        // Companions that receive a SoulState from an older Mac build (before ts
        // was added) must not panic. source and ts default to empty string.
        let raw = r#"{"type":"soul_state","day_label":"calm","index":85}"#;
        let back: ServerMessage = serde_json::from_str(raw).unwrap();
        match back {
            ServerMessage::SoulState { day_label, index, source, ts } => {
                assert_eq!(day_label, "calm");
                assert_eq!(index, 85);
                assert_eq!(source, "");
                assert_eq!(ts, "");
            }
            other => panic!("expected SoulState, got {other:?}"),
        }
    }
}
