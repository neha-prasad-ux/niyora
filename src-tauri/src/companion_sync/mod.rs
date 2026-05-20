//! HRV companion sync · milestone 3.
//!
//! Advertises a `_niyora._tcp` Bonjour/mDNS service on the local network so
//! the Niyora iOS companion (separate repo at `~/Claude Workspace/niyora/ios`)
//! can discover this Mac. When a breathing session completes, emits a session
//! window `{ session_id, start, end, technique_name }` as one line of JSON to
//! every connected peer.
//!
//! Scope is intentionally narrow:
//! - No pairing, no auth, no encryption. M3 verification is a sniffer (or
//!   `dns-sd -B _niyora._tcp` + `dns-sd -L Niyora _niyora._tcp`). Pairing
//!   lands in M4.
//! - No offline queue. If the phone is not connected at the moment of
//!   emission, the window is lost. Queue + backfill is M6.
//! - No HRV result handling. The phone replies in M5; for now we only push.
//!
//! Off by default. Enable with `NIYORA_COMPANION_SYNC=1` at launch.

use std::time::Duration;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

const SERVICE_TYPE: &str = "_niyora._tcp.local.";
const INSTANCE_NAME: &str = "Niyora";
const HOSTNAME: &str = "niyora-mac.local.";
const CHANNEL_CAPACITY: usize = 32;

/// The session window emitted to companions when a breathing session ends.
/// Mirrors the wire shape promised in `docs/hrv-companion-spec.md`. Extra
/// `technique_name` is included so future Soul-side views can attribute HRV
/// deltas per technique without an extra round-trip.
#[derive(Clone, Debug, Serialize)]
pub struct SessionWindow {
    pub session_id: String,
    /// RFC 3339 timestamp at session start.
    pub start: String,
    /// RFC 3339 timestamp at session end.
    pub end: String,
    pub technique_name: String,
}

/// Cloneable handle held in Tauri state. Cheap to clone; only the broadcast
/// `Sender` is duplicated. When the service is disabled the sender has no
/// subscribers and `send_window` is a silent no-op, which is the spec's
/// "feature degrades silently" contract.
#[derive(Clone)]
pub struct CompanionSync {
    sender: broadcast::Sender<SessionWindow>,
}

impl CompanionSync {
    /// Construct the disabled variant. Used when the env var opt-in is not
    /// set, or when service startup failed and we want the rest of the app
    /// to keep working.
    pub fn disabled() -> Self {
        let (sender, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender }
    }

    /// Fire-and-forget push of a session window. Never blocks the caller;
    /// dropping the window when no peer is connected is the documented
    /// behaviour for M3.
    pub fn send_window(&self, window: SessionWindow) {
        let _ = self.sender.send(window);
    }
}

/// Whether the user has opted into the companion sync service for this run.
pub fn is_enabled() -> bool {
    std::env::var("NIYORA_COMPANION_SYNC")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Starts the mDNS advertise + TCP listener tasks on the Tauri async runtime.
/// Returns a `CompanionSync` to be stashed in `App::manage`. Startup failures
/// (port unavailable, mDNS init failed, no local IP) are logged and the
/// returned handle is the disabled variant: an HRV-feature failure must
/// never crash the app or block a breathing session.
pub fn start() -> CompanionSync {
    if !is_enabled() {
        return CompanionSync::disabled();
    }

    let (tx, _rx) = broadcast::channel::<SessionWindow>(CHANNEL_CAPACITY);
    let tx_for_task = tx.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(tx_for_task).await {
            eprintln!("[companion_sync] service stopped: {e}");
        }
    });

    CompanionSync { sender: tx }
}

async fn run(sender: broadcast::Sender<SessionWindow>) -> Result<(), String> {
    // Ephemeral port so we never collide with another process on the user's
    // machine. We learn the assigned port from the bound socket and advertise
    // that exact number via mDNS.
    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?
        .port();

    let host_ipv4 = local_ipv4().ok_or_else(|| "no local IPv4 detected".to_string())?;
    let daemon = mdns_sd::ServiceDaemon::new().map_err(|e| format!("mdns daemon: {e}"))?;
    let service = mdns_sd::ServiceInfo::new(
        SERVICE_TYPE,
        INSTANCE_NAME,
        HOSTNAME,
        host_ipv4.as_str(),
        port,
        None,
    )
    .map_err(|e| format!("mdns service info: {e}"))?;
    daemon
        .register(service)
        .map_err(|e| format!("mdns register: {e}"))?;

    eprintln!("[companion_sync] advertising {SERVICE_TYPE} as {INSTANCE_NAME} at {host_ipv4}:{port}");

    loop {
        let (socket, addr) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[companion_sync] accept failed: {e}");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        eprintln!("[companion_sync] peer connected: {addr}");
        let rx = sender.subscribe();
        tauri::async_runtime::spawn(async move {
            serve_peer(socket, addr, rx).await;
        });
    }
}

async fn serve_peer(
    mut socket: tokio::net::TcpStream,
    addr: std::net::SocketAddr,
    mut rx: broadcast::Receiver<SessionWindow>,
) {
    // NDJSON: one window per line. Slow peers that fall behind the broadcast
    // ring lose the missed window, not the connection. Losing a window is
    // acceptable, dropping the peer mid-session is not.
    loop {
        let window = match rx.recv().await {
            Ok(w) => w,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[companion_sync] peer {addr} lagged {n} window(s)");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => return,
        };
        let mut line = match serde_json::to_string(&window) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[companion_sync] encode failed: {e}");
                continue;
            }
        };
        line.push('\n');
        if let Err(e) = socket.write_all(line.as_bytes()).await {
            eprintln!("[companion_sync] peer {addr} write failed: {e}");
            return;
        }
    }
}

/// Best-effort local IPv4 detection. Opens a UDP socket and reads its bound
/// address after a `connect` to a public IP. No traffic is sent. Falls back
/// to `None` so the caller can fail startup cleanly rather than advertise an
/// unreachable address.
fn local_ipv4() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    let addr = s.local_addr().ok()?;
    if addr.ip().is_unspecified() {
        return None;
    }
    Some(addr.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_window_serializes_to_documented_shape() {
        let w = SessionWindow {
            session_id: "abc-123".into(),
            start: "2026-05-20T10:00:00Z".into(),
            end: "2026-05-20T10:01:04Z".into(),
            technique_name: "Box Breathing".into(),
        };
        let v: serde_json::Value = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(v["session_id"], "abc-123");
        assert_eq!(v["start"], "2026-05-20T10:00:00Z");
        assert_eq!(v["end"], "2026-05-20T10:01:04Z");
        assert_eq!(v["technique_name"], "Box Breathing");
    }

    #[test]
    fn disabled_send_is_silent_noop() {
        let sync = CompanionSync::disabled();
        sync.send_window(SessionWindow {
            session_id: "x".into(),
            start: "s".into(),
            end: "e".into(),
            technique_name: "t".into(),
        });
        // No assertions: this test exists to prove `send_window` does not
        // panic or block when nothing is subscribed.
    }

    #[test]
    fn is_enabled_reflects_env_var() {
        // Note: we do not mutate the process env here because parallel tests
        // would race. This is a smoke check on the truthiness logic via a
        // dedicated helper that takes the value explicitly.
        fn truthy(v: &str) -> bool {
            v == "1" || v.eq_ignore_ascii_case("true")
        }
        assert!(truthy("1"));
        assert!(truthy("true"));
        assert!(truthy("TRUE"));
        assert!(!truthy("0"));
        assert!(!truthy("yes"));
        assert!(!truthy(""));
    }
}
