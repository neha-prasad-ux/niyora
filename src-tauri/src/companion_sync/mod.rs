//! HRV companion sync. macOS-only feature surface · provides a tiny no-op
//! shim on Windows so the cross-platform call sites (sessions.rs,
//! main.rs) don't need to be sprinkled with `#[cfg]` attributes.
//!
//! Real implementation lives in `macos_impl.rs` and the sibling modules
//! `protocol`, `keychain`, `state`, `queue`, `pairing`. See the doc
//! comment at the top of `macos_impl.rs` for the design.

/// A request the Mac wants to send to the phone · "capture a 30s PPG
/// reading for this side of this session." Cross-platform shape so call
/// sites can construct it without `#[cfg]`. Built by the React layer
/// when the user taps "Measure stress".
#[derive(Clone, Debug)]
pub struct MeasurementRequest {
    pub session_id: String,
    /// `"pre"` or `"post"`. Validated by the Mac before enqueue.
    pub phase: String,
    pub technique_name: String,
}

#[cfg(target_os = "macos")]
pub mod history;
#[cfg(target_os = "macos")]
pub mod keychain;
#[cfg(target_os = "macos")]
pub mod pairing;
#[cfg(target_os = "macos")]
pub mod protocol;
#[cfg(target_os = "macos")]
pub mod queue;
#[cfg(target_os = "macos")]
pub mod state;

#[cfg(target_os = "macos")]
pub mod commands;
#[cfg(target_os = "macos")]
mod macos_impl;
#[cfg(target_os = "macos")]
pub use macos_impl::start;

#[cfg(not(target_os = "macos"))]
mod stub {
    use super::MeasurementRequest;
    use tauri::AppHandle;

    #[derive(Clone, Default)]
    pub struct CompanionSync;

    impl CompanionSync {
        pub fn enqueue_request(&self, _: MeasurementRequest) {}
    }

    pub fn start(_app: AppHandle) -> CompanionSync {
        CompanionSync
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub::{start, CompanionSync};
