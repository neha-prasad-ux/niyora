// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_positioner::{Position, WindowExt};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Hide from dock — menu bar only
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Build the tray icon
            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray.png"))
                .expect("failed to load tray icon");

            TrayIconBuilder::with_id("tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Niyora — Breathe")
                .on_tray_icon_event(|tray, event| {
                    // Let the positioner plugin track tray position
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

                    // Toggle popover on left click
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                // Position below the tray icon, then show
                                let _ = window.as_ref().window().move_window(Position::TrayBottomCenter);
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Start background thread for breathing reminders
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                reminder_loop(app_handle);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Niyora");
}

/// Sends a notification reminder every 2 hours during work hours (9 AM - 6 PM).
fn reminder_loop(app: tauri::AppHandle) {
    use chrono::Local;
    use std::thread::sleep;
    use std::time::Duration;

    loop {
        let now = Local::now();
        let hour = now.hour();

        // Only notify during work hours
        if (9..18).contains(&hour) {
            let _ = app.notification()
                .builder()
                .title("Niyora")
                .body("Time for a breathing break. Click the menu bar icon to start.")
                .show();
        }

        // Sleep for 2 hours
        sleep(Duration::from_secs(2 * 60 * 60));
    }
}

use chrono::Timelike;
use tauri_plugin_notification::NotificationExt;
