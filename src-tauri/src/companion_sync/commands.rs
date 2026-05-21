//! Tauri commands exposed to the React Settings panel.
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
