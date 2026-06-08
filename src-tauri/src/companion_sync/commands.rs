//! Tauri commands exposed to the React Settings panel and breathing flow.
//!
//! Pairing is tap-and-approve: `companion_start_pairing` opens a discovery
//! window so the phone can find this Mac and connect; when a phone completes
//! the Noise handshake the backend fires a `companion://pairing-request`
//! event carrying the phone name and the SAS, and the panel calls
//! `companion_approve_pairing` once the user confirms the number matches.

use tauri::State;

use super::history::{self, HrvReading};
use super::macos_impl::{CompanionStatus, CompanionSync};
use super::MeasurementRequest;

#[tauri::command]
pub async fn companion_status(
    sync: State<'_, CompanionSync>,
) -> Result<CompanionStatus, String> {
    Ok(sync.status().await)
}

/// Open the pairing window. The Mac starts advertising and will surface an
/// approve prompt for the next unknown phone that connects.
#[tauri::command]
pub async fn companion_start_pairing(sync: State<'_, CompanionSync>) -> Result<(), String> {
    sync.start_pairing().await
}

/// User clicked Allow on a waiting prompt (after confirming the SAS matches).
#[tauri::command]
pub async fn companion_approve_pairing(
    client_id: String,
    sync: State<'_, CompanionSync>,
) -> Result<(), String> {
    sync.approve_pairing(&client_id).await
}

/// User dismissed a waiting prompt.
#[tauri::command]
pub async fn companion_reject_pairing(
    client_id: String,
    sync: State<'_, CompanionSync>,
) -> Result<(), String> {
    sync.reject_pairing(&client_id).await
}

#[tauri::command]
pub async fn companion_cancel_pairing(sync: State<'_, CompanionSync>) -> Result<(), String> {
    sync.cancel_pairing().await;
    Ok(())
}

#[tauri::command]
pub async fn companion_unpair(
    client_id: String,
    sync: State<'_, CompanionSync>,
) -> Result<(), String> {
    sync.unpair(&client_id).await
}

#[tauri::command]
pub fn companion_hrv_history() -> Vec<HrvReading> {
    history::load().readings
}

/// Tap-driven: the React layer calls this when the user hits "Measure
/// stress" on either the pre-session info screen (`phase = "pre"`) or
/// the post-session mood screen (`phase = "post"`). Validates the phase
/// and enqueues the request. Idempotent · the queue dedupes on
/// (session_id, phase) so a double-tap does not produce two frames.
#[tauri::command]
pub async fn companion_request_measurement(
    session_id: String,
    phase: String,
    technique_name: String,
    sync: State<'_, CompanionSync>,
) -> Result<(), String> {
    if phase != "pre" && phase != "post" {
        return Err(format!("invalid phase: {phase}"));
    }
    if session_id.trim().is_empty() {
        return Err("session_id required".into());
    }
    sync.enqueue_request(MeasurementRequest {
        session_id,
        phase,
        technique_name,
    });
    Ok(())
}

/// Dev-only · writes a synthetic pre+post pair into the companion HRV
/// history for `session_id` so the post-session reveal screen can be
/// previewed without going through the iPhone PPG flow. Gated to debug
/// builds; the command is not even compiled into release.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn companion_inject_synthetic_reveal(
    session_id: String,
    pre_rmssd_ms: f64,
    post_rmssd_ms: f64,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use chrono::Utc;
    use tauri::Emitter;
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let pre = history::PhaseCapture {
        rmssd_ms: Some(pre_rmssd_ms),
        sdnn_ms: Some(pre_rmssd_ms * 0.9),
        sample_count: 28,
        snr_db: Some(10.0),
        status: "ok".into(),
        received_at: now.clone(),
    };
    let post = history::PhaseCapture {
        rmssd_ms: Some(post_rmssd_ms),
        sdnn_ms: Some(post_rmssd_ms * 0.9),
        sample_count: 28,
        snr_db: Some(10.0),
        status: "ok".into(),
        received_at: now,
    };
    history::merge(&session_id, history::Phase::Pre, pre)?;
    history::merge(&session_id, history::Phase::Post, post)?;
    let _ = app.emit("companion://state", serde_json::json!({"synthetic": true}));
    Ok(())
}
