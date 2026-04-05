// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener, Manager,
};
use tauri_plugin_positioner::{Position, WindowExt};
use chrono::Timelike;
use tauri_plugin_notification::NotificationExt;

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    cocoa::appkit::{NSMainMenuWindowLevel, NSWindowCollectionBehavior},
    panel_delegate, ManagerExt, WebviewWindowExt,
};

const NSPANEL_STYLE_MASK_NON_ACTIVATING: i32 = 1 << 7;

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let panel = app_handle.get_webview_panel("main").unwrap();
        panel.order_out(None);
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![hide_panel])
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_nspanel::init())
        .setup(|app| {
            // Hide from dock — menu bar only
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Convert the main window to an NSPanel for proper popover behavior
            #[cfg(target_os = "macos")]
            setup_panel(app.handle());

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
                        toggle_panel(tray.app_handle());
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

/// Convert the main window into an NSPanel with proper menubar popover behavior.
#[cfg(target_os = "macos")]
fn setup_panel(app_handle: &tauri::AppHandle) {
    let window = app_handle
        .get_webview_window("main")
        .expect("no window labeled 'main'");

    // Create a delegate that fires an event when the panel loses focus
    let delegate = panel_delegate!(NiyoraPanelDelegate {
        window_did_resign_key
    });

    let handle = app_handle.clone();
    delegate.set_listener(Box::new(move |event_name: String| {
        if event_name.as_str() == "window_did_resign_key" {
            let _ = handle.emit("panel_did_resign_key", ());
        }
    }));

    // Convert window to NSPanel
    let panel = window.to_panel().unwrap();

    // Float above the menu bar
    panel.set_level(NSMainMenuWindowLevel + 1);

    // Can appear on all Spaces, stay in place during Mission Control, work with fullscreen
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );

    // Non-activating: won't steal focus from other apps
    panel.set_style_mask(NSPANEL_STYLE_MASK_NON_ACTIVATING);

    // Set the delegate for resign-key notifications
    panel.set_delegate(delegate);

    // Auto-hide when the panel loses focus (click outside)
    let handle = app_handle.clone();
    app_handle.listen("panel_did_resign_key", move |_| {
        let panel = handle.get_webview_panel("main").unwrap();
        if panel.is_visible() {
            panel.order_out(None);
        }
    });
}

/// Toggle the panel: show positioned below tray, or hide.
#[cfg(target_os = "macos")]
fn toggle_panel(app_handle: &tauri::AppHandle) {
    let panel = app_handle.get_webview_panel("main").unwrap();

    if panel.is_visible() {
        panel.order_out(None);
        return;
    }

    // Position below the tray icon using the positioner plugin
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.move_window(Position::TrayBottomCenter);
    }

    panel.show();
}

/// Non-macOS fallback (won't compile on other platforms without this).
#[cfg(not(target_os = "macos"))]
fn toggle_panel(_app_handle: &tauri::AppHandle) {}

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
