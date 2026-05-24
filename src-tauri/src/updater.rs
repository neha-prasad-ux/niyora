// Auto-update flow for Niyora.
//
// Privacy contract (see /privacy/ on the marketing site): the app makes only
// three kinds of outbound request, and nothing else.
//  1. Update checks: a small JSON fetch to the endpoint configured in
//     tauri.conf.json (downloads.niyora.com/latest.json). No telemetry is
//     attached. This happens:
//     - Once at launch (all builds)
//     - Every ~24h (±2h jitter) via a periodic background check (release only)
//     - When the user clicks "Check for Updates..." in the tray menu
//  2. Update downloads: when a check from (1) finds a newer version, the app
//     fetches the full signed binary from downloads.niyora.com. On a periodic
//     check this happens silently in the background; on a manual or launch
//     check the user is prompted first. No telemetry is attached.
//  3. Anonymous analytics events, but only if the user opted in on the
//     onboarding consent slide. See telemetry.rs for exactly what is sent.
//
// Behavior:
// - On launch, `check(app, false)` is called once. If a new version is found,
//   the user gets a notification with "Update now" / "Later". No notification
//   is shown if they're already on the latest version.
// - `start_periodic_check` re-checks every ~24h (jittered ±2h). If an update
//   is found, it downloads and installs silently with no restart or UI.
// - The "Check for Updates..." tray menu item calls `check(app, true)`, which
//   additionally shows a "You're up to date" notification when there is no
//   update available, so the user gets immediate feedback for a manual click.

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

/// Spawns a background thread that re-checks for updates every ~24h while the
/// app is running. Jittered by +/-2h to avoid thundering herd across installs.
/// Silent: downloads and installs the update in the background with no
/// user-visible UI or restart. The new version takes effect on the next
/// natural app launch.
///
/// Release builds only. In debug builds this is a no-op so dev runs don't
/// produce spurious network traffic.
#[cfg(not(debug_assertions))]
pub fn start_periodic_check(app: AppHandle) {
    use rand::Rng;
    use std::time::Duration;

    std::thread::spawn(move || {
        let base_secs: u64 = 24 * 60 * 60; // 24h
        let jitter_range: u64 = 2 * 60 * 60; // +/-2h

        loop {
            let jitter: i64 = rand::thread_rng().gen_range(-(jitter_range as i64)..=(jitter_range as i64));
            let sleep_secs = (base_secs as i64 + jitter).max(60) as u64;
            std::thread::sleep(Duration::from_secs(sleep_secs));

            let app = app.clone();
            // If the async runtime has shut down (app teardown), spawn panics.
            // Catch that to break out of the loop instead of spinning forever.
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tauri::async_runtime::spawn(async move {
                    let updater = match app.updater() {
                        Ok(u) => u,
                        Err(e) => {
                            eprintln!("periodic updater init failed: {e}");
                            return;
                        }
                    };
                    match updater.check().await {
                        Ok(Some(update)) => {
                            if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
                                eprintln!("periodic update install failed: {e}");
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("periodic update check failed: {e}");
                        }
                    }
                });
            }));
            if ok.is_err() {
                eprintln!("periodic updater: async runtime gone, stopping loop");
                break;
            }
        }
    });
}

#[cfg(debug_assertions)]
pub fn start_periodic_check(_app: AppHandle) {}


pub fn check(app: AppHandle, prompt_when_current: bool) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(u) => u,
            Err(e) => {
                eprintln!("updater init failed: {e}");
                return;
            }
        };

        match updater.check().await {
            Ok(Some(update)) => {
                prompt_user(&app, update);
            }
            Ok(None) => {
                if prompt_when_current {
                    show_simple(&app, "Niyora is up to date.", "");
                }
            }
            Err(e) => {
                eprintln!("update check failed: {e}");
                if prompt_when_current {
                    show_simple(
                        &app,
                        "Could not check for updates.",
                        "Check your internet connection and try again.",
                    );
                }
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn prompt_user(app: &AppHandle, update: tauri_plugin_updater::Update) {
    let app = app.clone();
    let version = update.version.clone();
    let body = if !update.body.clone().unwrap_or_default().trim().is_empty() {
        update.body.clone().unwrap_or_default()
    } else {
        "A new version of Niyora is available.".to_string()
    };

    std::thread::spawn(move || {
        use mac_notification_sys::{MainButton, Notification, NotificationResponse};

        let title = format!("Niyora {version} is available");
        let response = Notification::new()
            .title(&title)
            .message(&body)
            .main_button(MainButton::SingleAction("Update now"))
            .close_button("Later")
            .send();

        let response = match response {
            Ok(r) => r,
            Err(_) => return,
        };

        match response {
            NotificationResponse::ActionButton(_) | NotificationResponse::Click => {
                install(app, update);
            }
            _ => {}
        }
    });
}

/// Windows prompt: plain toast without inline buttons (no Update/Later, see
/// project memory). v1 gap: there is no in-app install trigger yet, so the
/// toast is informational only. v1.1 will add an in-panel "Install update"
/// button that calls `install` from the frontend.
#[cfg(not(target_os = "macos"))]
fn prompt_user(app: &AppHandle, update: tauri_plugin_updater::Update) {
    use tauri_plugin_notification::NotificationExt;
    let title = format!("Niyora {} is available", update.version);
    let body = if !update.body.clone().unwrap_or_default().trim().is_empty() {
        update.body.clone().unwrap_or_default()
    } else {
        "Open Niyora to install. (Auto-update from a toast is coming soon.)".to_string()
    };
    let _ = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show();
}

fn install(app: AppHandle, update: tauri_plugin_updater::Update) {
    tauri::async_runtime::spawn(async move {
        match update.download_and_install(|_, _| {}, || {}).await {
            Ok(_) => {
                show_simple(&app, "Niyora updated. Relaunching…", "");
                app.restart();
            }
            Err(e) => {
                eprintln!("update install failed: {e}");
                show_simple(
                    &app,
                    "Update failed.",
                    "Please download the latest version from niyora.com.",
                );
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn show_simple(_app: &AppHandle, title: &str, body: &str) {
    let _ = mac_notification_sys::Notification::new()
        .title(title)
        .message(body)
        .send();
}

#[cfg(not(target_os = "macos"))]
fn show_simple(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}
