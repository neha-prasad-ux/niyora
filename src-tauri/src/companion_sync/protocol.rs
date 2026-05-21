//! Wire protocol v1 between the Niyora Mac app and the Niyora Companion
//! iPhone app. Each frame is one line of JSON over TCP (NDJSON). Every
//! message carries a `type` discriminator. Unknown types are a fatal error
//! and the connection is dropped, on the principle that a misaligned client
//! is more dangerous than a disconnect.
//!
//! Lifecycle from the phone's perspective:
//!
//! ```text
//!   →  identify   {client_id, client_name, pairing_id?}
//!   ←  hello      {server_id, server_name}
//!   ←  challenge  {nonce_hex}
//!   →  auth       {hmac_hex}                     // HMAC-SHA256(secret, nonce)
//!   ←  authed     or   ← auth_failed (then close)
//!   ←  window     {session_id, start, end, technique_name}   (one per session, queued or live)
//! ```
//!
//! `pairing_id` is only set on the first connect after a QR scan. The Mac
//! holds the QR-issued secret and pairing_id in memory and binds them to
//! the phone's `client_id` on receipt. Subsequent reconnects omit the
//! `pairing_id` and auth via stored secret.

use serde::{Deserialize, Serialize};

/// Wire-format protocol version. Bumped if any message shape changes in a
/// way that is not additive (renamed field, removed field, changed type).
pub const PROTOCOL_VERSION: u32 = 1;

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
    /// HRV computed by the phone for a session window the Mac previously
    /// sent. One per session_id. Fields are optional so the phone can
    /// honestly report "no data this time" without inventing numbers,
    /// matching the spec's "honest gaps" principle.
    HrvResult {
        session_id: String,
        /// Mean SDNN over the pre-session window, in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        pre_ms: Option<f64>,
        /// Mean SDNN over the post-session window, in milliseconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        post_ms: Option<f64>,
        /// post_ms - pre_ms. Convenience for the Mac so My Soul does not
        /// have to compute the same arithmetic.
        #[serde(skip_serializing_if = "Option::is_none")]
        delta_ms: Option<f64>,
        sample_counts: HrvSampleCounts,
        /// "ok" when both windows had samples; "no_data" otherwise.
        status: String,
    },
}

/// Number of HRV samples that fell in each window. The Mac uses these to
/// decide whether a delta is trustworthy (1 sample either side is noisy).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HrvSampleCounts {
    pub pre: u32,
    pub post: u32,
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
    /// One session window, either freshly produced or drained from the
    /// queue. Idempotent: `session_id` is stable, the phone may receive
    /// the same window twice across reconnects (queue replay) and must
    /// dedupe by id.
    Window {
        session_id: String,
        start: String,
        end: String,
        technique_name: String,
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
            protocol: 1,
            client_id: "c-1".into(),
            client_name: "Neha's iPhone".into(),
            pairing_id: Some("p-1".into()),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "identify");
        assert_eq!(v["client_id"], "c-1");
        assert_eq!(v["pairing_id"], "p-1");
        assert_eq!(v["protocol"], 1);
    }

    #[test]
    fn identify_without_pairing_id_omits_field() {
        let m = ClientMessage::Identify {
            protocol: 1,
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
                protocol: 1,
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
            ServerMessage::Window {
                session_id: "sess-1".into(),
                start: "2026-05-20T10:00:00Z".into(),
                end: "2026-05-20T10:01:00Z".into(),
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
    fn hrv_result_serializes_with_optional_fields() {
        let m = ClientMessage::HrvResult {
            session_id: "sess-1".into(),
            pre_ms: Some(42.5),
            post_ms: Some(48.0),
            delta_ms: Some(5.5),
            sample_counts: HrvSampleCounts { pre: 3, post: 4 },
            status: "ok".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["type"], "hrv_result");
        assert_eq!(v["session_id"], "sess-1");
        assert_eq!(v["pre_ms"], 42.5);
        assert_eq!(v["post_ms"], 48.0);
        assert_eq!(v["delta_ms"], 5.5);
        assert_eq!(v["sample_counts"]["pre"], 3);
        assert_eq!(v["sample_counts"]["post"], 4);
        assert_eq!(v["status"], "ok");
    }

    #[test]
    fn hrv_result_no_data_omits_numeric_fields() {
        let m = ClientMessage::HrvResult {
            session_id: "sess-2".into(),
            pre_ms: None,
            post_ms: None,
            delta_ms: None,
            sample_counts: HrvSampleCounts { pre: 0, post: 0 },
            status: "no_data".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("pre_ms").is_none());
        assert!(v.get("post_ms").is_none());
        assert!(v.get("delta_ms").is_none());
        assert_eq!(v["status"], "no_data");
    }

    #[test]
    fn hrv_result_round_trips_through_decode() {
        let m = ClientMessage::HrvResult {
            session_id: "sess-3".into(),
            pre_ms: Some(40.0),
            post_ms: Some(45.0),
            delta_ms: Some(5.0),
            sample_counts: HrvSampleCounts { pre: 2, post: 3 },
            status: "ok".into(),
        };
        let s = serde_json::to_string(&m).unwrap();
        let back: ClientMessage = serde_json::from_str(&s).unwrap();
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
