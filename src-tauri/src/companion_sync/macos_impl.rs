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
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Serialize;
use snow::TransportState;
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};

use super::history::{Phase, PhaseCapture};
use super::pairing::{PairingManager, PairingRequest};
use super::protocol::{ClientMessage, ServerMessage, PROTOCOL_VERSION, PROTOCOL_VERSION_MIN};
use super::queue::QueuedRequest;
use super::state::PairedDevice;
use super::{history, keychain, noise, pairing, queue, state, MeasurementRequest};

const SERVICE_TYPE: &str = "_niyora._tcp.local.";
const INSTANCE_NAME: &str = "Niyora";
const HOSTNAME: &str = "niyora-mac.local.";
const CHANNEL_CAPACITY: usize = 32;
const STATE_EVENT: &str = "companion://state";
const PAIRING_REQUEST_EVENT: &str = "companion://pairing-request";

/// Public shape echoed to the React Settings panel via Tauri commands.
#[derive(Clone, Debug, Serialize)]
pub struct CompanionStatus {
    pub running: bool,
    pub server_name: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub paired_devices: Vec<PairedDevice>,
    /// True while the pairing window is open (user tapped "Pair").
    pub pairing_active: bool,
    pub pairing_seconds_remaining: Option<u64>,
    /// Phones that have handshaked during the window and are waiting on the
    /// user to click Allow. Each carries the SAS to show next to the prompt.
    pub pending_requests: Vec<PairingRequest>,
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

    /// Open the pairing window so the Mac will prompt to approve an unknown
    /// phone. Boots the TCP listener and mDNS advertisement on first call so
    /// the phone can discover the Mac and connect. No QR, no secret · trust
    /// comes from the Noise handshake plus the human clicking Allow.
    pub async fn start_pairing(&self) -> Result<(), String> {
        self.ensure_listener_started().await?;
        {
            let mut inner = self.inner.lock().await;
            inner.pairing.open_window();
        }
        self.emit_state().await;
        Ok(())
    }

    pub async fn cancel_pairing(&self) {
        {
            let mut inner = self.inner.lock().await;
            inner.pairing.close_window();
        }
        self.emit_state().await;
    }

    /// User clicked Allow on a waiting prompt. Resolves the approval so the
    /// phone's `serve_peer` task completes the pairing.
    pub async fn approve_pairing(&self, client_id: &str) -> Result<(), String> {
        let found = {
            let mut inner = self.inner.lock().await;
            inner.pairing.resolve(client_id, true)
        };
        if !found {
            return Err("no pending request for that device".into());
        }
        self.emit_state().await;
        Ok(())
    }

    /// User dismissed a waiting prompt.
    pub async fn reject_pairing(&self, client_id: &str) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.pairing.resolve(client_id, false);
        }
        self.emit_state().await;
        Ok(())
    }

    pub async fn unpair(&self, client_id: &str) -> Result<(), String> {
        keychain::delete_peer_pubkey(client_id)?;
        state::remove(client_id)?;
        self.emit_state().await;
        Ok(())
    }

    pub async fn status(&self) -> CompanionStatus {
        let inner = self.inner.lock().await;
        let pairing_active = inner.pairing.window_active();
        let pairing_seconds_remaining = inner.pairing.time_remaining().map(|d| d.as_secs());
        let pending_requests = inner.pairing.pending_requests();
        CompanionStatus {
            running: inner.port.is_some(),
            server_name: inner.server_name.clone(),
            host: inner.host.clone(),
            port: inner.port,
            paired_devices: state::load().devices,
            pairing_active,
            pairing_seconds_remaining,
            pending_requests,
        }
    }

    /// Register a pending approval, surface it to the UI, and await the
    /// human's decision (bounded by the window TTL). Returns true if the
    /// user clicked Allow. The inner lock is never held across the await, so
    /// the approve/reject commands can resolve it.
    async fn await_approval(
        &self,
        client_id: &str,
        client_name: &str,
        sas: &str,
    ) -> Result<bool, String> {
        let rx = {
            let mut inner = self.inner.lock().await;
            match inner.pairing.request_approval(client_id, client_name, sas) {
                Ok(rx) => rx,
                // Window not open · refuse a drive-by connect silently.
                Err(_) => return Ok(false),
            }
        };
        let app = self.inner.lock().await.app.clone();
        let _ = app.emit(
            PAIRING_REQUEST_EVENT,
            PairingRequest {
                client_id: client_id.to_string(),
                client_name: client_name.to_string(),
                sas: sas.to_string(),
            },
        );
        self.emit_state().await;

        let decided = matches!(
            tokio::time::timeout(pairing::PAIRING_TTL, rx).await,
            Ok(Ok(true))
        );
        if !decided {
            // Timed out or rejected · make sure nothing lingers in pending.
            let mut inner = self.inner.lock().await;
            inner.pairing.resolve(client_id, false);
        }
        self.emit_state().await;
        Ok(decided)
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
        // enable_addr_auto: the daemon tracks the host's IP addresses and
        // re-announces the service whenever they change. Without it the
        // advertised address is frozen at the IP captured here, so after a
        // network switch or wake-from-sleep the phone can no longer discover
        // the Mac until the app restarts.
        let service = mdns_sd::ServiceInfo::new(
            SERVICE_TYPE,
            INSTANCE_NAME,
            HOSTNAME,
            host.as_str(),
            port,
            None,
        )
        .map_err(|e| format!("mdns service info: {e}"))?
        .enable_addr_auto();
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
        mut socket: TcpStream,
        addr: std::net::SocketAddr,
    ) -> Result<(), String> {
        // 1. Noise XX handshake · the Mac is the responder. This seals the
        //    channel and yields both the peer's static key (identity) and the
        //    SAS the user compares against the phone's screen.
        let mac_priv = keychain::load_or_create_mac_identity()?;
        let mut hs = noise::handshake_responder(&mut socket, &mac_priv).await?;

        // 2. Identify, now sealed. No pairing_id, no HMAC · the static key
        //    the handshake established is the identity.
        let identify_pt = noise::recv_encrypted(&mut socket, &mut hs.transport).await?;
        let identify: ClientMessage = serde_json::from_slice(&identify_pt)
            .map_err(|e| format!("identify decode: {e}"))?;
        let (client_id, client_name, peer_protocol) = match identify {
            ClientMessage::Identify {
                protocol,
                client_id,
                client_name,
            } => {
                if protocol < PROTOCOL_VERSION_MIN || protocol > PROTOCOL_VERSION {
                    send_msg(
                        &mut socket,
                        &mut hs.transport,
                        &ServerMessage::AuthFailed {
                            reason: format!("protocol {protocol} not supported"),
                        },
                    )
                    .await
                    .ok();
                    return Err(format!("client protocol={protocol}"));
                }
                (client_id, client_name, protocol)
            }
            other => return Err(format!("expected identify, got {other:?}")),
        };

        // 3. Hello · let the phone record this Mac's server_id/name for its
        //    own known-Macs list.
        let (server_id, server_name, app_handle) = {
            let inner = self.inner.lock().await;
            (
                inner.server_id.clone(),
                inner.server_name.clone(),
                inner.app.clone(),
            )
        };
        send_msg(
            &mut socket,
            &mut hs.transport,
            &ServerMessage::Hello {
                protocol: peer_protocol,
                server_id,
                server_name,
            },
        )
        .await?;

        // 4. Trust decision. A static key we have stored for this client_id
        //    and that matches what the handshake produced is a silent
        //    reconnect. Anything else (new phone, or a client_id presenting a
        //    different key) needs the window open + the human to click Allow
        //    after confirming the SAS matches.
        let stored = keychain::load_peer_pubkey(&client_id)?;
        let is_known = matches!(&stored, Some(k) if *k == hs.remote_static);
        let authed = if is_known {
            state::touch_last_seen(&client_id, &now_rfc3339()).ok();
            true
        } else {
            self.await_approval(&client_id, &client_name, &hs.sas).await?
        };

        if !authed {
            send_msg(
                &mut socket,
                &mut hs.transport,
                &ServerMessage::AuthFailed {
                    reason: "not approved".into(),
                },
            )
            .await
            .ok();
            return Err(format!("peer {addr} not approved"));
        }

        if !is_known {
            // Fresh pairing approved · persist the phone's static key and the
            // registry entry now, and close the window so a second phone
            // cannot ride the same approval.
            keychain::store_peer_pubkey(&client_id, &hs.remote_static)?;
            state::upsert(&client_id, &client_name, &now_rfc3339())?;
            let mut inner = self.inner.lock().await;
            inner.pairing.close_window();
        }

        send_msg(&mut socket, &mut hs.transport, &ServerMessage::Authed).await?;
        self.emit_state().await;
        eprintln!(
            "[companion_sync] peer {addr} authed client_id={client_id} ({client_name}) \
             protocol=v{peer_protocol} known={is_known}"
        );

        // 4b. Mac-side Soul tier + situational state. Min protocol is now 4,
        //     so every peer gets these.
        let stats = crate::sessions::load_session_stats();
        let tier = crate::sessions::current_soul_tier(stats.completed);
        send_msg(
            &mut socket,
            &mut hs.transport,
            &ServerMessage::StatusUpdate {
                soul_tier: tier.to_string(),
                completed_sessions: stats.completed,
                current_tier: tier.to_string(),
                total_session_count: stats.total,
                native_completed_sessions: stats.native_completed,
            },
        )
        .await?;
        let (soul_label, soul_index) = if let Some(sit) =
            app_handle.try_state::<crate::situational::SituationalStateHandle>()
        {
            let s = sit.0.lock().unwrap();
            (s.day_label.as_str().to_string(), s.niyora_index)
        } else {
            ("calm".to_string(), 100u8)
        };
        send_msg(
            &mut socket,
            &mut hs.transport,
            &ServerMessage::SoulState {
                day_label: soul_label,
                index: soul_index,
                source: "mac".to_string(),
                ts: now_rfc3339(),
            },
        )
        .await?;

        // 5. Drain the queue for this client_id.
        for entry in queue::pending_for(&client_id) {
            let msg = queue::to_request_message(&entry);
            if let Err(e) = send_msg(&mut socket, &mut hs.transport, &msg).await {
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

        // 6. Live loop. Split the socket so the pending read future borrows
        //    only the read half; the transport is borrowed synchronously
        //    inside each select branch (never across an await), so one task
        //    can both receive sealed frames and push sealed broadcasts.
        let (mut read_half, mut write_half) = socket.into_split();
        let mut transport = hs.transport;
        let mut rx = self.live_tx.subscribe();
        loop {
            tokio::select! {
                frame = noise::read_frame(&mut read_half) => {
                    let ciphertext = match frame {
                        Ok(f) => f,
                        Err(_) => return Ok(()),
                    };
                    let plaintext = match noise::open(&mut transport, &ciphertext) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("[companion_sync] open failed: {e}");
                            return Ok(());
                        }
                    };
                    if plaintext.is_empty() {
                        continue;
                    }
                    match serde_json::from_slice::<ClientMessage>(&plaintext) {
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
                            // Shared cooldown: a breath on the phone silences
                            // the Mac for the full interval, same as a breath
                            // on the Mac would. Also stamp phone activity for
                            // the active-device rule.
                            {
                                let app = self.inner.lock().await.app.clone();
                                if let Some(ls) =
                                    app.try_state::<crate::reminder::LastSessionTime>()
                                {
                                    *ls.0.lock().unwrap() = Instant::now();
                                }
                                if let Some(lp) =
                                    app.try_state::<crate::reminder::LastPhoneSessionTime>()
                                {
                                    *lp.0.lock().unwrap() = Some(Instant::now());
                                }
                            }
                            let stats = crate::sessions::load_session_stats();
                            let tier = crate::sessions::current_soul_tier(stats.completed);
                            if let Err(e) = send_msg(
                                &mut write_half,
                                &mut transport,
                                &ServerMessage::StatusUpdate {
                                    soul_tier: tier.to_string(),
                                    completed_sessions: stats.completed,
                                    current_tier: tier.to_string(),
                                    total_session_count: stats.total,
                                    native_completed_sessions: stats.native_completed,
                                },
                            )
                            .await
                            {
                                eprintln!("[companion_sync] status_update write failed: {e}");
                                return Ok(());
                            }
                            self.emit_state().await;
                        }
                        Ok(ClientMessage::PhoneActive { active, ts }) => {
                            eprintln!("[companion_sync] phone_active active={active} ts={ts}");
                            if active {
                                let app = self.inner.lock().await.app.clone();
                                if let Some(lp) =
                                    app.try_state::<crate::reminder::LastPhoneSessionTime>()
                                {
                                    *lp.0.lock().unwrap() = Some(Instant::now());
                                }
                            }
                        }
                        Ok(other) => {
                            eprintln!("[companion_sync] unexpected post-auth frame: {other:?}");
                        }
                        Err(e) => {
                            eprintln!("[companion_sync] post-auth decode failed: {e}");
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
                    match &msg {
                        ServerMessage::RequestMeasurement {
                            session_id,
                            phase,
                            ..
                        } => {
                            if let Err(e) =
                                send_msg(&mut write_half, &mut transport, &msg).await
                            {
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
                        ServerMessage::SoulState { .. } => {
                            if let Err(e) =
                                send_msg(&mut write_half, &mut transport, &msg).await
                            {
                                eprintln!("[companion_sync] live soul_state write failed: {e}");
                                return Ok(());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Broadcast the current situational soul-state to all live authed peers.
    /// Called by the situational score loop when the day label changes.
    pub fn broadcast_soul_state(&self, day_label: String, index: u8) {
        let _ = self.live_tx.send(ServerMessage::SoulState {
            day_label,
            index,
            source: "mac".to_string(),
            ts: now_rfc3339(),
        });
    }

    async fn emit_state(&self) {
        let status = self.status().await;
        let app = self.inner.lock().await.app.clone();
        let _ = app.emit(STATE_EVENT, status);
    }
}

/// Serialize a server message to JSON and write it sealed through the Noise
/// transport. Generic over the writer so it serves both the unsplit socket
/// (pre-loop handshake replies) and the split write half (live loop).
async fn send_msg<S>(
    stream: &mut S,
    transport: &mut TransportState,
    msg: &ServerMessage,
) -> Result<(), String>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    let bytes = serde_json::to_vec(msg).map_err(|e| e.to_string())?;
    noise::send_encrypted(stream, transport, &bytes).await
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

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
