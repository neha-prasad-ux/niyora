// Auto-update flow for Niyora.
//
// Privacy contract (see /privacy/ on the marketing site): the only outbound
// network request the app ever makes is this update check, hitting the
// endpoint configured in tauri.conf.json (downloads.niyora.com/latest.json).
// No telemetry or analytics is attached to the request.
//
// Behavior:
// - `maybe_silent_check` runs at launch and on every panel open, throttled to
//   at most once per 24h. If a new version is found we silently download and
//   swap the bundle on disk. The running process stays on the old binary; the
//   user picks up the new version next time they launch. No notification, no
//   forced restart.
// - The "Check for Updates…" tray menu item calls `check(app, true)`, which
//   prompts with "Update now" / "Later" and surfaces a "You're up to date"
//   notification when there's nothing new — explicit feedback for an explicit
//   user action.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const SILENT_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
static LAST_SILENT_CHECK: Mutex<Option<Instant>> = Mutex::new(None);

pub fn maybe_silent_check(app: AppHandle) {
    {
        let mut last = LAST_SILENT_CHECK.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < SILENT_CHECK_INTERVAL {
                return;
            }
        }
        *last = Some(Instant::now());
    }

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
                if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
                    eprintln!("silent update install failed: {e}");
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("silent update check failed: {e}"),
        }
    });
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
                    show_simple("Niyora is up to date.", "");
                }
            }
            Err(e) => {
                eprintln!("update check failed: {e}");
                if prompt_when_current {
                    show_simple(
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

#[cfg(not(target_os = "macos"))]
fn prompt_user(_: &AppHandle, _: tauri_plugin_updater::Update) {}

fn install(app: AppHandle, update: tauri_plugin_updater::Update) {
    tauri::async_runtime::spawn(async move {
        match update.download_and_install(|_, _| {}, || {}).await {
            Ok(_) => {
                show_simple(
                    "Niyora updated. Relaunching…",
                    "",
                );
                app.restart();
            }
            Err(e) => {
                eprintln!("update install failed: {e}");
                show_simple(
                    "Update failed.",
                    "Please download the latest version from niyora.com.",
                );
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn show_simple(title: &str, body: &str) {
    let _ = mac_notification_sys::Notification::new()
        .title(title)
        .message(body)
        .send();
}

#[cfg(not(target_os = "macos"))]
fn show_simple(_: &str, _: &str) {}
