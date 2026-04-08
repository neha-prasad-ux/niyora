// Niyora — macOS menu bar breathing & mindfulness app
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::Instant;

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

/// Shared timestamp of the last completed breathing session (or app launch).
/// Reset when user completes a session; checked by the reminder loop.
struct LastSessionTime(Arc<Mutex<Instant>>);

#[tauri::command]
fn resize_panel(app_handle: tauri::AppHandle, width: f64, height: f64) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
    }
}

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    // Reset the screen-time timer — user just completed a session
    if let Some(state) = app_handle.try_state::<LastSessionTime>() {
        *state.0.lock().unwrap() = Instant::now();
    }

    #[cfg(target_os = "macos")]
    {
        let panel = app_handle.get_webview_panel("main").unwrap();
        panel.order_out(None);
    }
}

fn main() {
    let last_session = Arc::new(Mutex::new(Instant::now()));

    tauri::Builder::default()
        .manage(LastSessionTime(last_session.clone()))
        .invoke_handler(tauri::generate_handler![hide_panel, resize_panel])
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_nspanel::init())
        .setup(move |app| {
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

            // Start background thread for smart screen-time reminders
            let app_handle = app.handle().clone();
            let last_session_for_thread = last_session.clone();
            std::thread::spawn(move || {
                reminder_loop(app_handle, last_session_for_thread);
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

    // Position below the tray icon and reload for a fresh session
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.move_window(Position::TrayBottomCenter);

        // Clamp to screen bounds so the panel doesn't go off-edge
        if let Ok(pos) = window.outer_position() {
            if let Ok(monitor) = window.current_monitor() {
                if let Some(monitor) = monitor {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    let win_size = window.outer_size().unwrap_or(tauri::Size::Logical(
                        tauri::LogicalSize { width: 420.0, height: 520.0 },
                    ).to_physical(scale));
                    let screen_w = screen.width as i32;
                    let win_w = win_size.width as i32;
                    let mut x = pos.x;
                    let y = pos.y;
                    // If panel extends past right edge, nudge left
                    if x + win_w > screen_w {
                        x = screen_w - win_w - 8;
                    }
                    // If panel extends past left edge, nudge right
                    if x < 0 {
                        x = 8;
                    }
                    let _ = window.set_position(tauri::Position::Physical(
                        tauri::PhysicalPosition { x, y },
                    ));
                }
            }
        }

        // Reload the webview to get a completely fresh React mount
        let _ = window.eval("window.location.reload()");
    }

    // Small delay to let reload start before showing
    std::thread::sleep(std::time::Duration::from_millis(50));
    panel.show();
}

/// Non-macOS fallback (won't compile on other platforms without this).
#[cfg(not(target_os = "macos"))]
fn toggle_panel(_app_handle: &tauri::AppHandle) {}

/// Smart screen-time reminder: notifies after 90 minutes of continuous screen time
/// during work hours (9 AM - 6 PM). Resets when the user completes a breathing session.
fn reminder_loop(app: tauri::AppHandle, last_session: Arc<Mutex<Instant>>) {
    use chrono::Local;
    use std::thread::sleep;
    use std::time::Duration;

    const SCREEN_TIME_LIMIT: Duration = Duration::from_secs(90 * 60); // 90 minutes
    const CHECK_INTERVAL: Duration = Duration::from_secs(60);         // check every minute
    const WORK_HOUR_START: u32 = 9;
    const WORK_HOUR_END: u32 = 18;

    let mut notified_for_current_period = false;

    loop {
        sleep(CHECK_INTERVAL);

        let now = Local::now();
        let hour = now.hour();

        // Only remind during work hours
        if !(WORK_HOUR_START..WORK_HOUR_END).contains(&hour) {
            continue;
        }

        let elapsed = last_session.lock().unwrap().elapsed();

        // If the timer was reset (session completed), clear the notification flag
        if elapsed < SCREEN_TIME_LIMIT {
            notified_for_current_period = false;
            continue;
        }

        // 90+ minutes elapsed — send one notification per period
        if !notified_for_current_period {
            let _ = app.notification()
                .builder()
                .title("Niyora")
                .body("You've been at your screen for 90 minutes. Take a breathing break?")
                .show();
            notified_for_current_period = true;
        }
    }
}
