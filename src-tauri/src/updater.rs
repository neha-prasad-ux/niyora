// Auto-update flow for Niyora.
//
// Privacy contract (see /privacy/ on the marketing site): the only outbound
// network request the app ever makes is this update check, hitting the
// endpoint configured in tauri.conf.json (downloads.niyora.com/latest.json).
// No telemetry or analytics is attached to the request.
//
// Behavior:
// - On launch, `check(app, false)` is called once. If a new version is found,
//   the user gets a notification with "Update now" / "Later". No notification
//   is shown if they're already on the latest version.
// - The "Check for Updates…" tray menu item calls `check(app, true)`, which
//   additionally shows a "You're up to date" notification when there is no
//   update available — so the user gets immediate feedback for a manual click.

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

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
