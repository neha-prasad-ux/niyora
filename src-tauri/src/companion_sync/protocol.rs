//! Wire protocol v3 between the Niyora Mac app and the Niyora Companion
//! iPhone app. Each frame is one line of JSON over TCP (NDJSON). Every
//! message carries a `type` discriminator. Unknown types are a fatal error
//! and the connection is dropped, on the principle that a misaligned client
//! is more dangerous than a disconnect.
//!
//! Lifecycle from the phone's perspective:
//!
//! ```text
//!   →  identify             {client_id, client_name, pairing_id?}
//!   ←  hello                {server_id, server_name}
//!   ←  challenge            {nonce_hex}
//!   →  auth                 {hmac_hex}                  // HMAC-SHA256(secret, nonce)
//!   ←  authed   or   ← auth_failed (then close)
//!   ←  status_update        {soul_tier, completed_sessions, current_tier, total_session_count}   (after authed)
//!   ←  soul_state           {day_label, index}                     (after authed, v3+)
//!   ←  request_measurement  {session_id, phase, technique_name}   (zero or more per session)
//!   →  session_recorded     {technique_name, duration_sec, completed, recorded_at}   (v3 only)
//! ```
//!
//! `pairing_id` is only set on the first connect after a QR scan. The Mac
//! holds the QR-issued secret and pairing_id in memory and binds them to
//! the phone's `client_id` on receipt. Subsequent reconnects omit the
//! `pairing_id` and auth via stored secret.
//!
//! v2 vs v1: the HealthKit-on-window auto-read path is gone. Sessions no
//! longer auto-emit a window at end. Instead the user explicitly opts in
//! to a 30s iPhone-camera PPG measurement via a "Measure stress" button,
//! and the Mac emits one `request_measurement` per tap (phase = pre or
//! post). The phone responds with `hrv_result` once it has computed RMSSD
//! and SDNN from the green-channel pulse it captured.
//!
//! v3 vs v2: adds `session_recorded` (phone -> Mac) so the iOS companion
//! can report completed breathing sessions for Soul tier progression.
//! Also adds `status_update` (Mac -> phone) so paired mode can show the
//! Mac's current Soul tier. v2 companions still connect and use HRV, but
//! cannot send `session_recorded` or receive `status_update`.

use serde::{Deserialize, Serialize};

/// Wire-format protocol version. Bumped on any breaking shape change
/// (renamed field, removed field, changed semantic). v3 adds
/// `SessionRecorded` (phone -> Mac) and `StatusUpdate` (Mac -> phone).
/// v2 companions are still accepted for HRV-only use.
pub const PROTOCOL_VERSION: u32 = 3;

/// Minimum protocol version accepted from companions. v2 clients can
/// still send HRV results but cannot use session sync.
pub const PROTOCOL_VERSION_MIN: u32 = 2;

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
    /// First frame on every connection. `pairing_id` is present only when
    /// the phone is using a fresh QR; on subsequent reconnects it is absent
    /// and the Mac looks up the secret by `client_id`.
    Identify {
        protocol: u32,
        client_id: String,
        client_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pairing_id: Option<String>,
    },
    /// Phone's response to the server `challenge`. `hmac_hex` is the lower
    /// case hex of `HMAC-SHA256(secret_bytes, nonce_bytes)` where the nonce
    /// is decoded from the hex string the server sent.
    Auth { hmac_hex: String },
    /// A completed (or abandoned) session from the iOS companion (v3+).
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
    /// Server-issued nonce. Phone replies with `auth { hmac_hex }`.
    Challenge { nonce_hex: String },
    /// Auth succeeded. Window frames may now flow.
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
    },
}

/// Payload encoded into the pairing QR code (base64 URL-safe, no padding).
/// Single-use: the Mac invalidates `pairing_id` after the first successful
/// HMAC verification, so a glimpsed QR cannot be reused.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QrPayload {
    pub v: u32,
    pub server_id: String,
    pub server_name: String,
    pub host: String,
    pub port: u16,
    pub pairing_id: String,
    pub secret_hex: String,
}

impl QrPayload {
    /// Encode self as base64-url JSON for inclusion in a QR code. Returns
    /// the string the iOS scanner decodes back into a `QrPayload`.
    pub fn encode(&self) -> Result<String, String> {
        use base64::Engine;
        let json = serde_json::to_vec(self).map_err(|e| format!("qr encode: {e}"))?;
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
    }

    /// Inverse of `encode`. Exposed (with `allow(dead_code)`) so the iOS
    /// team has a single Rust reference for the QR string format and the
    /// round-trip test below catches drift in either direction.
    #[allow(dead_code)]
    pub fn decode(s: &str) -> Result<Self, String> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|e| format!("qr decode b64: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("qr decode json: {e}"))
    }
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
            pairing_id: Some("p-1".into()),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "identify");
        assert_eq!(v["client_id"], "c-1");
        assert_eq!(v["pairing_id"], "p-1");
        assert_eq!(v["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn identify_without_pairing_id_omits_field() {
        let m = ClientMessage::Identify {
            protocol: PROTOCOL_VERSION,
            client_id: "c-1".into(),
            client_name: "Neha's iPhone".into(),
            pairing_id: None,
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
            ServerMessage::Challenge {
                nonce_hex: "deadbeef".into(),
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
            },
            ServerMessage::SoulState {
                day_label: "dense".into(),
                index: 42,
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
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "status_update");
        assert_eq!(v["soul_tier"], "radiance");
        assert_eq!(v["completed_sessions"], 42);
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
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "status_update");
        assert_eq!(v["current_tier"], "shine");
        assert_eq!(v["total_session_count"], 20);
        // Legacy fields must still be present for v3 companions.
        assert_eq!(v["soul_tier"], "shine");
        assert_eq!(v["completed_sessions"], 17);
    }

    #[test]
    fn soul_state_serializes_with_snake_case_tag() {
        let m = ServerMessage::SoulState {
            day_label: "dense".into(),
            index: 42,
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "soul_state");
        assert_eq!(v["day_label"], "dense");
        assert_eq!(v["index"], 42);
    }

    #[test]
    fn soul_state_round_trips() {
        let m = ServerMessage::SoulState {
            day_label: "heavy".into(),
            index: 15,
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: ServerMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn qr_payload_round_trips_through_base64() {
        let p = QrPayload {
            v: 1,
            server_id: "s-1".into(),
            server_name: "Neha's MacBook".into(),
            host: "192.168.1.42".into(),
            port: 51234,
            pairing_id: "p-1".into(),
            secret_hex: "00112233445566778899aabbccddeeff".into(),
        };
        let enc = p.encode().unwrap();
        let dec = QrPayload::decode(&enc).unwrap();
        assert_eq!(p, dec);
    }
}
