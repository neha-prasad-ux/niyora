use crate::config;

/// Returns true if the user has completed the first-launch onboarding flow.
#[tauri::command]
pub fn is_onboarded() -> bool {
    config::load().onboarded
}

/// Marks the onboarding flow as complete. Called from the frontend at the
/// end of the third slide when the user clicks Begin.
#[tauri::command]
pub fn mark_onboarded() -> Result<(), String> {
    let mut cfg = config::load();
    cfg.onboarded = true;
    config::save(&cfg)
}

/// Dev-only: resets the onboarded flag so the next panel open re-shows the
/// onboarding flow. Wired to a "Reset onboarding" tray menu item that only
/// appears in debug builds.
#[cfg(debug_assertions)]
pub fn reset_onboarded() -> Result<(), String> {
    let mut cfg = config::load();
    cfg.onboarded = false;
    config::save(&cfg)
}

/// Triggers the platform's notification permission dialog by attempting to
/// send a quiet welcome notification.
///
/// macOS surfaces its own native "Allow / Don't Allow" popup the first time
/// an app tries to notify; subsequent calls are silently allowed or denied
/// based on the user's choice.
///
/// Windows: installed desktop apps typically have notification permission
/// auto-granted via the AppUserModelID registered by the NSIS installer.
/// We still fire the welcome toast so the user sees confirmation that
/// reminders will appear.
#[tauri::command]
pub fn request_notification_permission(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use mac_notification_sys::Notification;
        let _ = &app;
        std::thread::spawn(|| {
            let _ = Notification::new()
                .title("Niyora")
                .message("You're all set. We'll nudge you when you need a break.")
                .send();
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Niyora")
            .body("You're all set. We'll nudge you when you need a break.")
            .show();
    }
    Ok(())
}
