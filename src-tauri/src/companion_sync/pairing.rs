//! Pairing window + pending-approval manager for the tap-and-approve flow.
//!
//! There is no QR and no pre-shared secret any more. Trust comes from two
//! human acts, gated here:
//!
//! 1. The Mac user opens the pair screen, which opens a time-boxed
//!    **pairing window**. Only while the window is open will the Mac prompt
//!    to approve an unknown phone. Outside the window an unknown phone is
//!    refused, so a device on the wifi cannot pop prompts unprompted.
//! 2. When an unknown phone completes the Noise handshake during the window,
//!    `serve_peer` registers a **pending approval** carrying the phone's
//!    name and the SAS, and awaits the human's decision. The Settings panel
//!    shows "<phone> wants to connect" plus the number; clicking Allow
//!    resolves the approval and the pairing completes.
//!
//! Nothing here is persisted. If the app restarts mid-pair the window is
//! gone and the user starts over, which is the right trade for a flow that
//! is only ever a few seconds long.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::oneshot;

/// How long the pairing window stays open after the user taps "Pair". Long
/// enough to pick the Mac on the phone and glance at the number, short
/// enough that a forgotten-open window closes itself.
pub const PAIRING_TTL: Duration = Duration::from_secs(120);

/// A phone that has handshaked during the window and is waiting on the human
/// to click Allow. The `decision` sender is consumed when the user decides;
/// dropping it (window closed, app quit) resolves the waiter as rejected.
struct PendingApproval {
    client_name: String,
    sas: String,
    decision: oneshot::Sender<bool>,
}

/// Public shape of a waiting request, surfaced to the Settings panel so a
/// reload re-renders the prompt even if it missed the live event.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PairingRequest {
    pub client_id: String,
    pub client_name: String,
    pub sas: String,
}

#[derive(Default)]
pub struct PairingManager {
    window_expires_at: Option<Instant>,
    pending: HashMap<String, PendingApproval>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open (or refresh) the pairing window.
    pub fn open_window(&mut self) {
        self.window_expires_at = Some(Instant::now() + PAIRING_TTL);
    }

    /// Close the window and reject every waiting approval. Called on cancel,
    /// and whenever a pairing completes so a second unknown phone cannot
    /// sneak in on the same window.
    pub fn close_window(&mut self) {
        self.window_expires_at = None;
        self.pending.clear(); // dropping each sender resolves its waiter as rejected
    }

    pub fn window_active(&self) -> bool {
        self.window_expires_at
            .map(|t| t > Instant::now())
            .unwrap_or(false)
    }

    /// Seconds left on the window, for the UI countdown.
    pub fn time_remaining(&self) -> Option<Duration> {
        let now = Instant::now();
        self.window_expires_at
            .filter(|t| *t > now)
            .map(|t| t.saturating_duration_since(now))
    }

    /// Register a phone waiting on approval and hand back the receiver
    /// `serve_peer` awaits. Fails if the window is not open · the caller
    /// turns that into an `auth_failed`. A second connect from the same
    /// `client_id` replaces the earlier wait (its sender drops, resolving it
    /// as rejected), which is what we want if a phone retries.
    pub fn request_approval(
        &mut self,
        client_id: &str,
        client_name: &str,
        sas: &str,
    ) -> Result<oneshot::Receiver<bool>, String> {
        if !self.window_active() {
            return Err("pairing window is not open".into());
        }
        let (tx, rx) = oneshot::channel();
        self.pending.insert(
            client_id.to_string(),
            PendingApproval {
                client_name: client_name.to_string(),
                sas: sas.to_string(),
                decision: tx,
            },
        );
        Ok(rx)
    }

    /// Resolve a waiting approval. Returns true if a matching request was
    /// found. The receiver side gets `approved`; on approve the caller
    /// persists the phone's static key and completes the pairing.
    pub fn resolve(&mut self, client_id: &str, approved: bool) -> bool {
        match self.pending.remove(client_id) {
            Some(p) => {
                let _ = p.decision.send(approved);
                true
            }
            None => false,
        }
    }

    /// Snapshot of every waiting request, for `companion_status`.
    pub fn pending_requests(&self) -> Vec<PairingRequest> {
        self.pending
            .iter()
            .map(|(client_id, p)| PairingRequest {
                client_id: client_id.clone(),
                client_name: p.client_name.clone(),
                sas: p.sas.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_opens_and_reports_active() {
        let mut m = PairingManager::new();
        assert!(!m.window_active());
        m.open_window();
        assert!(m.window_active());
        assert!(m.time_remaining().unwrap() <= PAIRING_TTL);
    }

    #[test]
    fn request_approval_requires_open_window() {
        let mut m = PairingManager::new();
        assert!(m.request_approval("c1", "Neha's iPhone", "123 456").is_err());
        m.open_window();
        assert!(m.request_approval("c1", "Neha's iPhone", "123 456").is_ok());
    }

    #[test]
    fn approve_resolves_the_waiter_true() {
        let mut m = PairingManager::new();
        m.open_window();
        let mut rx = m.request_approval("c1", "iPhone", "111 222").unwrap();
        assert!(m.resolve("c1", true));
        assert_eq!(rx.try_recv().unwrap(), true);
    }

    #[test]
    fn reject_resolves_the_waiter_false() {
        let mut m = PairingManager::new();
        m.open_window();
        let mut rx = m.request_approval("c1", "iPhone", "111 222").unwrap();
        assert!(m.resolve("c1", false));
        assert_eq!(rx.try_recv().unwrap(), false);
    }

    #[test]
    fn resolving_unknown_client_is_false() {
        let mut m = PairingManager::new();
        assert!(!m.resolve("nope", true));
    }

    #[test]
    fn closing_window_drops_waiters() {
        let mut m = PairingManager::new();
        m.open_window();
        let mut rx = m.request_approval("c1", "iPhone", "111 222").unwrap();
        m.close_window();
        assert!(!m.window_active());
        // Sender dropped without sending -> receiver errors (treated as reject).
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn pending_requests_lists_waiting_phones() {
        let mut m = PairingManager::new();
        m.open_window();
        let _rx = m.request_approval("c1", "Neha's iPhone", "123 456").unwrap();
        let reqs = m.pending_requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].client_id, "c1");
        assert_eq!(reqs[0].client_name, "Neha's iPhone");
        assert_eq!(reqs[0].sas, "123 456");
    }
}
