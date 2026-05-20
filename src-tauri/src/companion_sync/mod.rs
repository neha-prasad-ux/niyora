//! HRV companion sync. macOS-only feature surface · provides a tiny no-op
//! shim on Windows so the cross-platform call sites (sessions.rs,
//! main.rs) don't need to be sprinkled with `#[cfg]` attributes.
//!
//! Real implementation lives in `macos_impl.rs` and the sibling modules
//! `protocol`, `keychain`, `state`, `queue`, `pairing`. See the doc
//! comment at the top of `macos_impl.rs` for the design.

/// The session window emitted to the iPhone when a breathing session ends.
/// Cross-platform shape so call sites can construct it without `#[cfg]`.
#[derive(Clone, Debug)]
pub struct SessionWindow {
    pub session_id: String,
    pub start: String,
    pub end: String,
    pub technique_name: String,
}

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
pub use macos_impl::{start, CompanionStatus, CompanionSync};

#[cfg(not(target_os = "macos"))]
mod stub {
    use super::SessionWindow;
    use tauri::AppHandle;

    #[derive(Clone, Default)]
    pub struct CompanionSync;

    impl CompanionSync {
        pub fn enqueue_window(&self, _: SessionWindow) {}
    }

    pub fn start(_app: AppHandle) -> CompanionSync {
        CompanionSync
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub::{start, CompanionSync};
