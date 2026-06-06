//! HRV companion sync · milestone 4 (pairing + queue + auth gate).
//!
//! Responsibilities:
//!
//! - Bind a TCP listener and advertise it on the local network via mDNS as
//!   `_niyora._tcp`. Phones discover the Mac by name; the QR carries the
//!   exact endpoint so discovery is optional, not load-bearing.
//! - Manage pending QR pairings in memory and persist the resulting
//!   shared secret to the macOS Keychain on successful HMAC-SHA256 auth.
//! - Per-peer auth gate: window frames only flow to phones that have
//!   completed identify → challenge → auth → authed. Anything before that
//!   gets nothing.
//! - Persist unsent windows to `companion_queue.json` so a phone that
//!   reconnects later (or for the first time) catches up. Drain on auth.
//! - Tauri commands and a `companion://state` event so the React Settings
//!   panel can render the paired-devices list, pair button, and QR modal.
//!
//! Off by default for the user · the service binds and advertises only
//! after `start_pairing()` is invoked or a paired phone exists. This keeps
//! the network footprint zero for users who never use the HRV feature.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::Serialize;
use sha2::Sha256;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};

use super::history::{Phase, PhaseCapture};
use super::pairing::PairingManager;
use super::protocol::{ClientMessage, QrPayload, ServerMessage, PROTOCOL_VERSION, PROTOCOL_VERSION_MIN};
use super::queue::QueuedRequest;
use super::state::PairedDevice;
use super::{history, keychain, queue, state, MeasurementRequest};

const SERVICE_TYPE: &str = "_niyora._tcp.local.";
const INSTANCE_NAME: &str = "Niyora";
const HOSTNAME: &str = "niyora-mac.local.";
const CHANNEL_CAPACITY: usize = 32;
const STATE_EVENT: &str = "companion://state";

type HmacSha256 = Hmac<Sha256>;

/// Public shape echoed to the React Settings panel via Tauri commands.
#[derive(Clone, Debug, Serialize)]
pub struct CompanionStatus {
    pub running: bool,
    pub server_name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub paired_devices: Vec<PairedDevice>,
    pub pairing_active: bool,
    pub pairing_seconds_remaining: Option<u64>,
}

/// Cloneable handle held in Tauri state. The inner Arc<Mutex<..>> holds
/// pairing manager state and the latest QR endpoint; the broadcast Sender
/// is a separate field so live `enqueue_request` calls are lock-free.
#[derive(Clone)]
pub struct CompanionSync {
    inner: Arc<Mutex<Inner>>,
    live_tx: broadcast::Sender<ServerMessage>,
}

struct Inner {
    app: AppHandle,
    server_id: String,
    server_name: String,
    host: Option<String>,
    port: Option<u16>,
    pairing: PairingManager,
    /// Held to keep the mDNS advertisement alive · dropping the daemon
    /// withdraws the service.
    mdns_daemon: Option<mdns_sd::ServiceDaemon>,
}

impl CompanionSync {
    /// Persist a measurement request to the on-disk queue and try to fan
    /// it out to any live authed peers. Never blocks the caller. The
    /// queue is the source of truth · the broadcast is just a fast path
    /// for the case where a phone happens to be connected. Invalid
    /// `phase` values are dropped silently · the frontend validates
    /// before it ever gets here.
    pub fn enqueue_request(&self, req: MeasurementRequest) {
        if Phase::parse(&req.phase).is_none() {
            eprintln!("[companion_sync] enqueue_request: bad phase {:?}", req.phase);
            return;
        }
        let qw = QueuedRequest {
            session_id: req.session_id.clone(),
            phase: req.phase.clone(),
            technique_name: req.technique_name.clone(),
            queued_at: now_rfc3339(),
            sent_to: Default::default(),
        };
        if let Err(e) = queue::enqueue(qw) {
            eprintln!("[companion_sync] queue enqueue failed: {e}");
        }
        let _ = self.live_tx.send(ServerMessage::RequestMeasurement {
            session_id: req.session_id,
            phase: req.phase,
            technique_name: req.technique_name,
        });
    }

    /// Start (or refresh) a pairing. Boots the TCP listener and mDNS
    /// advertisement on first call. Returns the QR payload the Settings
    /// panel renders.
    pub async fn start_pairing(&self) -> Result<QrPayload, String> {
        self.ensure_listener_started().await?;
        let mut inner = self.inner.lock().await;
        let host = inner
            .host
            .clone()
            .ok_or_else(|| "listener has no host".to_string())?;
        let port = inner
            .port
            .ok_or_else(|| "listener has no port".to_string())?;
        let server_id = inner.server_id.clone();
        let server_name = inner.server_name.clone();
        inner.pairing.cancel_all();
        let qr = inner.pairing.start(&server_id, &server_name, &host, port);
        drop(inner);
        self.emit_state().await;
        Ok(qr)
    }

    pub async fn cancel_pairing(&self) {
        let mut inner = self.inner.lock().await;
        inner.pairing.cancel_all();
        drop(inner);
        self.emit_state().await;
    }

    pub async fn unpair(&self, client_id: &str) -> Result<(), String> {
        keychain::delete_secret(client_id)?;
        state::remove(client_id)?;
        self.emit_state().await;
        Ok(())
    }

    pub async fn status(&self) -> CompanionStatus {
        let inner = self.inner.lock().await;
        let pairing_active = inner.pairing.active_count() > 0;
        let pairing_seconds_remaining = inner
            .pairing
            .time_remaining()
            .map(|d| d.as_secs());
        CompanionStatus {
            running: inner.port.is_some(),
            server_name: inner.server_name.clone(),
            host: inner.host.clone(),
            port: inner.port,
            paired_devices: state::load().devices,
            pairing_active,
            pairing_seconds_remaining,
        }
    }

    /// Start the listener + mDNS advertise lazily. Called by start_pairing
    /// and (on app start) if there is already at least one paired device
    /// so reconnects work without user action.
    pub async fn ensure_listener_started(&self) -> Result<(), String> {
        {
            let inner = self.inner.lock().await;
            if inner.port.is_some() {
                return Ok(());
            }
        }
        let listener = TcpListener::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("bind: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("local_addr: {e}"))?
            .port();
        let host = local_ipv4().ok_or_else(|| "no local IPv4 detected".to_string())?;

        let daemon = mdns_sd::ServiceDaemon::new().map_err(|e| format!("mdns daemon: {e}"))?;
        let service = mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            INSTANCE_NAME,
            HOSTNAME,
            host.as_str(),
            port,
            None,
        )
        .map_err(|e| format!("mdns service info: {e}"))?;
        daemon
            .register(service)
            .map_err(|e| format!("mdns register: {e}"))?;

        {
            let mut inner = self.inner.lock().await;
            inner.host = Some(host.clone());
            inner.port = Some(port);
            inner.mdns_daemon = Some(daemon);
        }

        let sync = self.clone();
        tauri::async_runtime::spawn(async move {
            sync.run_accept_loop(listener).await;
        });
        eprintln!("[companion_sync] listener up on {host}:{port}");
        self.emit_state().await;
        Ok(())
    }

    async fn run_accept_loop(self, listener: TcpListener) {
        loop {
            let (socket, addr) = match listener.accept().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[companion_sync] accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };
            let sync = self.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = sync.serve_peer(socket, addr).await {
                    eprintln!("[companion_sync] peer {addr} ended: {e}");
                }
            });
        }
    }

    async fn serve_peer(
        self,
        socket: TcpStream,
        addr: std::net::SocketAddr,
    ) -> Result<(), String> {
        let (read_half, mut write_half) = socket.into_split();
        let mut reader = BufReader::new(read_half);
        let mut line = String::new();

        // 1. Identify
        line.clear();
        let n = reader.read_line(&mut line).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("peer closed before identify".into());
        }
        let identify: ClientMessage =
            serde_json::from_str(line.trim()).map_err(|e| format!("identify decode: {e}"))?;
        let (client_id, client_name, pairing_id, peer_protocol) = match identify {
            ClientMessage::Identify {
                protocol,
                client_id,
                client_name,
                pairing_id,
            } => {
                if protocol < PROTOCOL_VERSION_MIN || protocol > PROTOCOL_VERSION {
                    write_message(
                        &mut write_half,
                        &ServerMessage::AuthFailed {
                            reason: format!("protocol {} not supported", protocol),
                        },
                    )
                    .await
                    .ok();
                    return Err(format!("client protocol={protocol}"));
                }
                (client_id, client_name, pairing_id, protocol)
            }
            other => return Err(format!("expected identify, got {other:?}")),
        };

        // 2. Resolve the secret: either bind a fresh pairing or look up
        // the secret of a previously-paired client.
        let (server_id, server_name, app_handle) = {
            let inner = self.inner.lock().await;
            (inner.server_id.clone(), inner.server_name.clone(), inner.app.clone())
        };
        // Echo the negotiated peer protocol back to the client rather than
        // PROTOCOL_VERSION. v2 companions that check exact equality on the
        // Hello will otherwise reject every connection after we ship v3.
        write_message(
            &mut write_half,
            &ServerMessage::Hello {
                protocol: peer_protocol,
                server_id,
                server_name,
            },
        )
        .await?;

        let (secret, is_fresh_pairing) = if let Some(pid) = pairing_id.as_ref() {
            let mut inner = self.inner.lock().await;
            match inner.pairing.consume(pid) {
                Some(s) => (s, true),
                None => {
                    drop(inner);
                    write_message(
                        &mut write_half,
                        &ServerMessage::AuthFailed {
                            reason: "pairing_id unknown or expired".into(),
                        },
                    )
                    .await
                    .ok();
                    return Err("bad pairing_id".into());
                }
            }
        } else {
            match keychain::load_secret(&client_id) {
                Ok(Some(s)) => (s, false),
                Ok(None) => {
                    write_message(
                        &mut write_half,
                        &ServerMessage::AuthFailed {
                            reason: "not paired".into(),
                        },
                    )
                    .await
                    .ok();
                    return Err("unknown client_id".into());
                }
                Err(e) => return Err(e),
            }
        };

        // 3. Challenge + Auth
        let mut nonce_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce_hex = hex(&nonce_bytes);
        write_message(
            &mut write_half,
            &ServerMessage::Challenge {
                nonce_hex: nonce_hex.clone(),
            },
        )
        .await?;

        line.clear();
        let n = reader.read_line(&mut line).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("peer closed before auth".into());
        }
        let auth_msg: ClientMessage =
            serde_json::from_str(line.trim()).map_err(|e| format!("auth decode: {e}"))?;
        let provided_hmac_hex = match auth_msg {
            ClientMessage::Auth { hmac_hex } => hmac_hex,
            other => return Err(format!("expected auth, got {other:?}")),
        };

        let mut mac = HmacSha256::new_from_slice(&secret)
            .map_err(|e| format!("hmac key: {e}"))?;
        mac.update(&nonce_bytes);
        let expected_tag = mac.finalize().into_bytes();
        let expected_hex = hex(&expected_tag);
        if !constant_time_eq(provided_hmac_hex.as_bytes(), expected_hex.as_bytes()) {
            write_message(
                &mut write_half,
                &ServerMessage::AuthFailed {
                    reason: "bad hmac".into(),
                },
            )
            .await
            .ok();
            return Err("hmac mismatch".into());
        }

        // 4. Auth succeeded. On fresh pairing, persist the secret + the
        // registry entry now (not before) so a failed pair leaves no trace.
        if is_fresh_pairing {
            keychain::store_secret(&client_id, &secret)?;
            state::upsert(&client_id, &client_name, &now_rfc3339())?;
        } else {
            state::touch_last_seen(&client_id, &now_rfc3339()).ok();
        }
        write_message(&mut write_half, &ServerMessage::Authed).await?;
        self.emit_state().await;
        eprintln!(
            "[companion_sync] peer {addr} authed as client_id={client_id} ({client_name}) protocol=v{peer_protocol}"
        );

        // 4b. Send current Soul tier + session count to v3+ companions
        // so paired mode can display Mac-side progression.
        if peer_protocol >= 3 {
            let stats = crate::sessions::load_session_stats();
            let tier = crate::sessions::current_soul_tier(stats.completed);
            write_message(
                &mut write_half,
                &ServerMessage::StatusUpdate {
                    soul_tier: tier.to_string(),
                    completed_sessions: stats.completed,
                    current_tier: tier.to_string(),
                    total_session_count: stats.total,
                },
            )
            .await?;

            // Send current situational soul-state. Only the derived label +
            // index cross the wire; raw collector inputs stay local.
            let (soul_label, soul_index) = if let Some(sit) = app_handle
                .try_state::<crate::situational::SituationalStateHandle>()
            {
                let s = sit.0.lock().unwrap();
                (s.day_label.as_str().to_string(), s.niyora_index)
            } else {
                ("calm".to_string(), 100u8)
            };
            write_message(
                &mut write_half,
                &ServerMessage::SoulState {
                    day_label: soul_label,
                    index: soul_index,
                },
            )
            .await?;
        }

        // 5. Drain the queue for this client_id.
        for entry in queue::pending_for(&client_id) {
            let msg = queue::to_request_message(&entry);
            if let Err(e) = write_message(&mut write_half, &msg).await {
                eprintln!("[companion_sync] drain write failed: {e}");
                return Ok(());
            }
            let paired_ids: Vec<String> = state::load()
                .devices
                .iter()
                .map(|d| d.client_id.clone())
                .collect();
            queue::mark_sent_to(&entry.session_id, &entry.phase, &client_id, &paired_ids).ok();
            state::touch_window_sent(&client_id, &now_rfc3339()).ok();
        }
        self.emit_state().await;

        // 6. Live loop. Two things happen concurrently:
        //   - We push freshly tapped measurement requests from the
        //     broadcast.
        //   - We receive `hrv_result` frames from the phone for previously
        //     sent requests. Each frame is merged into the per-session
        //     history row by (session_id, phase).
        let mut rx = self.live_tx.subscribe();
        let mut incoming = String::new();
        loop {
            tokio::select! {
                read = reader.read_line(&mut incoming) => {
                    match read {
                        Ok(0) => return Ok(()),
                        Ok(_) => {
                            let trimmed = incoming.trim();
                            if trimmed.is_empty() {
                                incoming.clear();
                                continue;
                            }
                            match serde_json::from_str::<ClientMessage>(trimmed) {
                                Ok(ClientMessage::HrvResult {
                                    session_id,
                                    phase,
                                    rmssd_ms,
                                    sdnn_ms,
                                    sample_count,
                                    snr_db,
                                    status,
                                }) => {
                                    eprintln!(
                                        "[companion_sync] hrv_result session={session_id} \
                                         phase={phase} rmssd={rmssd_ms:?} sdnn={sdnn_ms:?} \
                                         samples={sample_count} snr_db={snr_db:?} status={status}"
                                    );
                                    let Some(p) = Phase::parse(&phase) else {
                                        eprintln!("[companion_sync] bad phase {phase:?}");
                                        incoming.clear();
                                        continue;
                                    };
                                    let capture = PhaseCapture {
                                        rmssd_ms,
                                        sdnn_ms,
                                        sample_count,
                                        snr_db,
                                        status,
                                        received_at: now_rfc3339(),
                                    };
                                    if let Err(e) = history::merge(&session_id, p, capture) {
                                        eprintln!("[companion_sync] history merge failed: {e}");
                                    }
                                    self.emit_state().await;
                                }
                                Ok(ClientMessage::SessionRecorded {
                                    technique_name,
                                    technique_kind,
                                    duration_sec,
                                    intended_duration_sec,
                                    completed,
                                    recorded_at,
                                }) => {
                                    if peer_protocol < 3 {
                                        eprintln!("[companion_sync] session_recorded from v{peer_protocol} peer, ignoring");
                                        incoming.clear();
                                        continue;
                                    }
                                    let intended = intended_duration_sec.unwrap_or(duration_sec);
                                    eprintln!(
                                        "[companion_sync] session_recorded technique={technique_name} \
                                         kind={technique_kind} intended={intended}s actual={duration_sec}s \
                                         completed={completed} at={recorded_at}"
                                    );
                                    if let Err(e) = crate::sessions::record_companion_session(
                                        &technique_name,
                                        &technique_kind,
                                        intended,
                                        duration_sec,
                                        completed,
                                        &recorded_at,
                                    ) {
                                        eprintln!("[companion_sync] session persist failed: {e}");
                                    }
                                    // Send updated tier info back to the companion.
                                    let stats = crate::sessions::load_session_stats();
                                    let tier = crate::sessions::current_soul_tier(stats.completed);
                                    if let Err(e) = write_message(
                                        &mut write_half,
                                        &ServerMessage::StatusUpdate {
                                            soul_tier: tier.to_string(),
                                            completed_sessions: stats.completed,
                                            current_tier: tier.to_string(),
                                            total_session_count: stats.total,
                                        },
                                    )
                                    .await
                                    {
                                        eprintln!("[companion_sync] status_update write failed: {e}");
                                        return Ok(());
                                    }
                                    self.emit_state().await;
                                }
                                Ok(other) => {
                                    eprintln!("[companion_sync] unexpected post-auth frame: {other:?}");
                                }
                                Err(e) => {
                                    eprintln!("[companion_sync] post-auth decode failed: {e}");
                                    return Ok(());
                                }
                            }
                            incoming.clear();
                        }
                        Err(e) => {
                            eprintln!("[companion_sync] post-auth read failed: {e}");
                            return Ok(());
                        }
                    }
                }
                recv = rx.recv() => {
                    let msg = match recv {
                        Ok(m) => m,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("[companion_sync] peer {addr} lagged {n} message(s)");
                            continue;
                        }
                        Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    };
                    if let ServerMessage::RequestMeasurement {
                        ref session_id,
                        ref phase,
                        ..
                    } = msg
                    {
                        if let Err(e) = write_message(&mut write_half, &msg).await {
                            eprintln!("[companion_sync] live write failed: {e}");
                            return Ok(());
                        }
                        let paired_ids: Vec<String> = state::load()
                            .devices
                            .iter()
                            .map(|d| d.client_id.clone())
                            .collect();
                        queue::mark_sent_to(session_id, phase, &client_id, &paired_ids).ok();
                        state::touch_window_sent(&client_id, &now_rfc3339()).ok();
                        self.emit_state().await;
                    }
                    if let ServerMessage::SoulState { .. } = &msg {
                        if peer_protocol >= 3 {
                            if let Err(e) = write_message(&mut write_half, &msg).await {
                                eprintln!("[companion_sync] live soul_state write failed: {e}");
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    /// Broadcast the current situational soul-state to all live authed peers.
    /// Called by the situational score loop when the day label changes.
    pub fn broadcast_soul_state(&self, day_label: String, index: u8) {
        let _ = self.live_tx.send(ServerMessage::SoulState { day_label, index });
    }

    async fn emit_state(&self) {
        let status = self.status().await;
        let app = self.inner.lock().await.app.clone();
        let _ = app.emit(STATE_EVENT, status);
    }
}

async fn write_message(
    write_half: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &ServerMessage,
) -> Result<(), String> {
    let mut line = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    line.push('\n');
    write_half
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Public entry point. Mints (or loads) the stable `server_id`, but does
/// not bind a TCP port until either a paired device exists or the user
/// initiates pairing. Returns a handle even on partial failure so the rest
/// of the app keeps working.
pub fn start(app: AppHandle) -> CompanionSync {
    let mut cfg = crate::config::load();
    let server_id = match cfg.companion_server_id.clone() {
        Some(id) => id,
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            cfg.companion_server_id = Some(id.clone());
            if let Err(e) = crate::config::save(&cfg) {
                eprintln!("[companion_sync] could not persist server_id: {e}");
            }
            id
        }
    };

    let server_name = default_server_name();

    let (live_tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
    let sync = CompanionSync {
        inner: Arc::new(Mutex::new(Inner {
            app: app.clone(),
            server_id,
            server_name,
            host: None,
            port: None,
            pairing: PairingManager::new(),
            mdns_daemon: None,
        })),
        live_tx,
    };

    // If there is already a paired phone, start the listener now so
    // reconnects work without user action. Otherwise stay dormant.
    let has_paired = !state::load().devices.is_empty();
    if has_paired {
        let s = sync.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = s.ensure_listener_started().await {
                eprintln!("[companion_sync] auto-start listener failed: {e}");
            }
        });
    }

    sync
}

fn default_server_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::process::Command::new("scutil")
                .args(["--get", "ComputerName"])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8(o.stdout).ok()?;
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
        })
        .unwrap_or_else(|| "Niyora Mac".to_string())
}

fn local_ipv4() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    let addr = s.local_addr().ok()?;
    if addr.ip().is_unspecified() {
        return None;
    }
    Some(addr.ip().to_string())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Constant-time comparison · returns true iff both slices have the same
/// length and every byte matches. Avoids leaking the prefix-match length
/// of an HMAC tag through timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_verify_round_trip() {
        let secret = b"some 32 byte secret bytes here !!".to_vec();
        let nonce = b"a nonce".to_vec();

        let mut mac = HmacSha256::new_from_slice(&secret).unwrap();
        mac.update(&nonce);
        let tag = mac.finalize().into_bytes();
        let tag_hex = hex(&tag);

        let mut mac2 = HmacSha256::new_from_slice(&secret).unwrap();
        mac2.update(&nonce);
        let tag2 = mac2.finalize().into_bytes();
        assert_eq!(tag_hex, hex(&tag2));
    }

    #[test]
    fn hmac_verify_rejects_wrong_secret() {
        let nonce = b"shared nonce";
        let mut a = HmacSha256::new_from_slice(b"key-a").unwrap();
        a.update(nonce);
        let mut b = HmacSha256::new_from_slice(b"key-b").unwrap();
        b.update(nonce);
        assert_ne!(hex(&a.finalize().into_bytes()), hex(&b.finalize().into_bytes()));
    }

    #[test]
    fn constant_time_eq_matches_plain_eq_semantics() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn hex_round_trip_for_known_bytes() {
        assert_eq!(hex(&[0, 1, 15, 16, 255]), "00010f10ff");
    }
}
