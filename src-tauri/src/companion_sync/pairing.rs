//! In-memory pairing manager. When the user taps "Pair your iPhone" in
//! Settings, we mint a fresh `PendingPairing` containing:
//!
//! - a random 16-byte `pairing_id` (single-use)
//! - a random 32-byte `secret` (the eventual HMAC key, never written to
//!   disk until a phone consumes the pairing successfully)
//! - the host/port the TCP listener bound to
//! - an expiry `Instant`
//!
//! Lifecycle:
//!
//! 1. `start()` returns a `QrPayload` to render on the Mac.
//! 2. The phone scans the QR, connects, sends `identify { pairing_id }`.
//! 3. `consume(pairing_id, client_id, client_name)` removes the pending
//!    entry, writes the secret to Keychain keyed by `client_id`, registers
//!    the device in `state.rs`, and returns the secret so the running
//!    connection can keep using it for HMAC verification.
//! 4. `cancel()` and `tick()` evict expired entries.
//!
//! Pending pairings are not persisted · if the app restarts mid-pairing,
//! the user starts over. That's the right trade · persistence would mean
//! a secret-in-progress sits on disk, weakening the "secret never leaves
//! memory until tied to a phone" property.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use rand::RngCore;

use super::protocol::QrPayload;

pub const PAIRING_TTL: Duration = Duration::from_secs(90);

#[derive(Debug, Clone)]
pub struct PendingPairing {
    pub secret: Vec<u8>,
    pub expires_at: Instant,
}

#[derive(Debug, Default)]
pub struct PairingManager {
    pending: HashMap<String, PendingPairing>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Mint a fresh pairing. The QR payload includes everything the phone
    /// needs · server identity, endpoint, pairing_id, secret.
    pub fn start(
        &mut self,
        server_id: &str,
        server_name: &str,
        host: &str,
        port: u16,
    ) -> QrPayload {
        let mut rng = rand::thread_rng();

        let mut pairing_bytes = [0u8; 16];
        rng.fill_bytes(&mut pairing_bytes);
        let pairing_id = hex(&pairing_bytes);

        let mut secret_bytes = [0u8; 32];
        rng.fill_bytes(&mut secret_bytes);
        let secret_hex = hex(&secret_bytes);

        self.pending.insert(
            pairing_id.clone(),
            PendingPairing {
                secret: secret_bytes.to_vec(),
                expires_at: Instant::now() + PAIRING_TTL,
            },
        );

        QrPayload {
            v: 1,
            server_id: server_id.to_string(),
            server_name: server_name.to_string(),
            host: host.to_string(),
            port,
            pairing_id,
            secret_hex,
        }
    }

    /// Phone presented this `pairing_id`. If it is still valid and unused,
    /// remove it from pending and return the bound secret. The caller is
    /// responsible for writing the secret to the Keychain against the
    /// phone's `client_id` and recording the pairing in `state.rs`.
    pub fn consume(&mut self, pairing_id: &str) -> Option<Vec<u8>> {
        self.tick();
        self.pending.remove(pairing_id).map(|p| p.secret)
    }

    pub fn cancel_all(&mut self) {
        self.pending.clear();
    }

    /// Drop expired entries. Cheap enough to run on every consume() and on
    /// every command-handler call · we are talking about at most a handful
    /// of pending entries at a time.
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.pending.retain(|_, v| v.expires_at > now);
    }

    pub fn active_count(&self) -> usize {
        self.pending.len()
    }

    /// Time remaining for any active pairing, for the UI countdown. We
    /// return the maximum across all pending entries · in practice there
    /// is only ever one, since the Settings UI calls `cancel_all` before
    /// `start`.
    pub fn time_remaining(&self) -> Option<Duration> {
        let now = Instant::now();
        self.pending
            .values()
            .map(|p| p.expires_at.saturating_duration_since(now))
            .max()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_returns_full_qr_payload_and_registers_pending() {
        let mut m = PairingManager::new();
        let qr = m.start("server-1", "Neha's MacBook", "10.0.0.5", 51000);
        assert_eq!(qr.server_id, "server-1");
        assert_eq!(qr.host, "10.0.0.5");
        assert_eq!(qr.port, 51000);
        assert_eq!(qr.pairing_id.len(), 32, "16 random bytes hex-encoded");
        assert_eq!(qr.secret_hex.len(), 64, "32 random bytes hex-encoded");
        assert_eq!(m.active_count(), 1);
    }

    #[test]
    fn consume_removes_pending_and_returns_secret() {
        let mut m = PairingManager::new();
        let qr = m.start("s", "s name", "h", 1);
        let secret = m.consume(&qr.pairing_id).expect("present");
        assert_eq!(secret.len(), 32);
        assert_eq!(m.active_count(), 0);
        assert!(m.consume(&qr.pairing_id).is_none(), "single-use");
    }

    #[test]
    fn unknown_pairing_id_is_none() {
        let mut m = PairingManager::new();
        m.start("s", "s name", "h", 1);
        assert!(m.consume("never-issued").is_none());
    }

    #[test]
    fn cancel_all_clears_pending() {
        let mut m = PairingManager::new();
        m.start("s", "s name", "h", 1);
        m.start("s", "s name", "h", 1);
        assert_eq!(m.active_count(), 2);
        m.cancel_all();
        assert_eq!(m.active_count(), 0);
    }

    #[test]
    fn tick_evicts_expired() {
        let mut m = PairingManager::new();
        // Insert one entry that expired one second ago.
        let id = "expired-id".to_string();
        m.pending.insert(
            id,
            PendingPairing {
                secret: vec![0; 32],
                expires_at: Instant::now() - Duration::from_secs(1),
            },
        );
        assert_eq!(m.active_count(), 1);
        m.tick();
        assert_eq!(m.active_count(), 0);
    }

    #[test]
    fn distinct_pairings_have_distinct_ids_and_secrets() {
        let mut m = PairingManager::new();
        let a = m.start("s", "s", "h", 1);
        let b = m.start("s", "s", "h", 1);
        assert_ne!(a.pairing_id, b.pairing_id);
        assert_ne!(a.secret_hex, b.secret_hex);
    }
}
