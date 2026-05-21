//! Tauri commands exposed to the React Settings panel and breathing flow.
//!
//! The frontend never sees raw `QrPayload` · the backend renders both
//! the base64 payload and an SVG of the QR code so the UI just drops in
//! an `<img>`/`<div dangerouslySetInnerHTML>` with no JS QR library.

use qrcode::render::svg;
use qrcode::QrCode;
use serde::Serialize;
use tauri::State;

use super::history::{self, HrvReading};
use super::macos_impl::{CompanionStatus, CompanionSync};
use super::MeasurementRequest;

#[derive(Debug, Clone, Serialize)]
pub struct PairingQr {
    /// Base64 of the QR JSON payload. Exposed for tests and so the iOS
    /// app team can verify decode parity from a screenshot.
    pub payload_b64: String,
    /// Inline SVG of the QR. Ready to render with
    /// `<div dangerouslySetInnerHTML={{ __html: qr_svg }} />`.
    pub qr_svg: String,
    pub seconds_remaining: u64,
}

#[tauri::command]
pub async fn companion_status(
    sync: State<'_, CompanionSync>,
) -> Result<CompanionStatus, String> {
    Ok(sync.status().await)
}

#[tauri::command]
pub async fn companion_start_pairing(
    sync: State<'_, CompanionSync>,
) -> Result<PairingQr, String> {
    let qr = sync.start_pairing().await?;
    let payload_b64 = qr.encode()?;
    let code = QrCode::new(payload_b64.as_bytes()).map_err(|e| format!("qr: {e}"))?;
    let qr_svg = code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .dark_color(svg::Color("#111"))
        .light_color(svg::Color("#fff"))
        .build();
    let status = sync.status().await;
    Ok(PairingQr {
        payload_b64,
        qr_svg,
        seconds_remaining: status.pairing_seconds_remaining.unwrap_or(0),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_svg_renders_for_a_payload() {
        let code = QrCode::new(b"hello world").unwrap();
        let svg = code
            .render::<svg::Color>()
            .min_dimensions(100, 100)
            .build();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }
}
