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

/// Spawns a tokio task that re-checks for updates every ~24h while the app is
/// running. Jittered by +/-2h to avoid thundering herd across installs.
/// Silent: downloads and installs the update in the background with no
/// user-visible UI or restart. The new version takes effect on the next
/// natural app launch.
///
/// Lifecycle: the task runs on `tauri::async_runtime`, so it is automatically
/// aborted when the runtime shuts down (e.g. on `app.exit(0)` from the tray
/// Quit menu). No manual shutdown signal needed.
///
/// First periodic check fires after a 10-minute warm-up rather than 24 hours,
/// so an update released within the first day of an install gets adopted on
/// the same day instead of the day after. The launch-time `check()` still
/// runs separately at startup, so the warm-up is purely a fallback for
/// updates that land during the first 24 hours of uptime.
///
/// Release builds only. In debug builds this is a no-op so dev runs don't
/// produce spurious network traffic.
#[cfg(not(debug_assertions))]
pub fn start_periodic_check(app: AppHandle) {
    use std::time::Duration;

    tauri::async_runtime::spawn(async move {
        // Short warm-up before the first periodic round so the launch-time
        // `check()` has a clean window to finish on cold start.
        tokio::time::sleep(Duration::from_secs(10 * 60)).await;

        loop {
            run_one_check(&app).await;
            sleep_jittered(24 * 60 * 60, 2 * 60 * 60).await;
        }
    });
}

#[cfg(debug_assertions)]
pub fn start_periodic_check(_app: AppHandle) {}

#[cfg(not(debug_assertions))]
async fn run_one_check(app: &AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[updater] periodic init failed: {e}");
            return;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            // Coarse milestone logging at every 25% so a stalled download
            // is visible in stderr without spamming the log on a typical
            // 100MB install. Metered-network detection is a known gap (see
            // privacy contract above); platform-specific NWPathMonitor /
            // Windows NetworkInformation integration is tracked separately.
            let progress = |downloaded: usize, total: Option<u64>| {
                if let Some(total) = total.filter(|&t| t > 0) {
                    let curr_pct = (downloaded as f64 / total as f64 * 100.0) as u32;
                    let prev = downloaded.saturating_sub(1);
                    let prev_pct = (prev as f64 / total as f64 * 100.0) as u32;
                    if curr_pct / 25 != prev_pct / 25 {
                        eprintln!(
                            "[updater] periodic download progress: {curr_pct}% \
                             ({downloaded}/{total} bytes)"
                        );
                    }
                }
            };
            let on_finished = || eprintln!("[updater] periodic download complete");
            if let Err(e) = update.download_and_install(progress, on_finished).await {
                eprintln!("[updater] periodic install failed: {e}");
            }
        }
        Ok(None) => {}
        Err(e) => {
            eprintln!("[updater] periodic check failed: {e}");
        }
    }
}

#[cfg(not(debug_assertions))]
async fn sleep_jittered(base_secs: u64, jitter_range_secs: u64) {
    use rand::Rng;
    use std::time::Duration;
    let jitter: i64 = rand::thread_rng()
        .gen_range(-(jitter_range_secs as i64)..=(jitter_range_secs as i64));
    // Floor at 60s so the loop never tight-spins if `base_secs` is ever
    // changed to a smaller value during testing.
    let sleep_secs = (base_secs as i64 + jitter).max(60) as u64;
    tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
}


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
