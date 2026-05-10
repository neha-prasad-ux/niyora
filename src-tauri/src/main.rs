// Niyora — macOS menu bar breathing & mindfulness app
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analytics;
mod config;
mod onboarding;
mod reminder;
mod sessions;
mod situational;
#[cfg(target_os = "macos")]
mod tray_mac;
mod updater;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener, Manager,
};

/// Last-known tray icon position+size, captured from tray events.
/// Used by `toggle_panel` to anchor the panel directly under the tray icon
/// in screen coordinates — more deterministic than letting the positioner
/// plugin compute it after the window has been hidden/moved.
struct TrayRect(Arc<Mutex<Option<(f64, f64, f64, f64)>>>);

/// True while the user is mid breathing session. Used to suppress the
/// click-outside-to-dismiss behavior so a stray click doesn't abort their
/// 60-second practice. Set from the frontend on Begin / Done.
struct SessionActive(Arc<Mutex<bool>>);

#[tauri::command]
fn set_session_active(active: bool, state: tauri::State<'_, SessionActive>) {
    *state.0.lock().unwrap() = active;
}

/// Visual state of the menu-bar icon.
///
/// `Measuring` is the default while the app is running: a green electron on
/// the atom signals "we're watching your screen-time signals." `Reminder`
/// swaps to an amber electron when a notification has fired and the user
/// hasn't acted on it yet; clicking the panel, snoozing, or accepting the
/// notification returns the icon to `Measuring`.
///
/// The icon is rendered as two layers: a monochrome template body (ring +
/// nucleus) that macOS auto-tints per-pixel against whatever's behind it on
/// the menu bar, and a coloured electron NSImageView composited on top.
/// See `tray_mac.rs` for the AppKit plumbing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayState {
    Measuring,
    Reminder,
}

/// The current tray state, kept in app-managed state so background callers
/// (e.g. `reminder.rs`) can transition without holding their own copy.
pub(crate) struct CurrentTrayState(pub Arc<Mutex<TrayState>>);

pub(crate) fn set_tray_state(app: &tauri::AppHandle, state: TrayState) {
    if let Some(s) = app.try_state::<CurrentTrayState>() {
        *s.0.lock().unwrap() = state;
    }

    #[cfg(target_os = "macos")]
    {
        let h = app.clone();
        let _ = app.run_on_main_thread(move || {
            tray_mac::apply_state(state, None);
            // Hint to the reader: `h` is here so the closure owns an
            // AppHandle clone, matching the run_on_main_thread signature
            // used elsewhere in this file.
            let _ = h;
        });
    }
}

use crate::reminder::{LastSessionTime, SnoozedUntil};
use crate::situational::{SituationalState, SituationalStateHandle};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    cocoa::appkit::{NSMainMenuWindowLevel, NSWindowCollectionBehavior},
    panel_delegate, ManagerExt, WebviewWindowExt,
};

const NSPANEL_STYLE_MASK_NON_ACTIVATING: i32 = 1 << 7;

#[tauri::command]
fn resize_panel(app_handle: tauri::AppHandle, width: f64, height: f64) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
    }
}

#[tauri::command]
fn hide_panel(app_handle: tauri::AppHandle) {
    if let Some(state) = app_handle.try_state::<LastSessionTime>() {
        *state.0.lock().unwrap() = Instant::now();
    }

    #[cfg(target_os = "macos")]
    {
        let panel = app_handle.get_webview_panel("main").unwrap();
        panel.order_out(None);
    }
}

/// If the app is running from a mounted DMG (path under /Volumes/), spawn
/// a detached helper that copies the bundle to /Applications and relaunches
/// it from there, then exit immediately. Doing the copy synchronously here
/// makes macOS Launch Services time out ("Niyora is not responding") because
/// the app never gets a chance to register a run loop.
///
/// Skipped in debug builds so `pnpm tauri dev` from any working directory
/// keeps working.
#[cfg(target_os = "macos")]
fn relocate_from_dmg_if_needed() {
    if cfg!(debug_assertions) {
        return;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    if !exe.to_string_lossy().starts_with("/Volumes/") {
        return;
    }
    // Walk up to find the .app bundle root. current_exe is at:
    //   /Volumes/Niyora/Niyora.app/Contents/MacOS/niyora
    let app_bundle = match exe
        .ancestors()
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("app"))
    {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    let app_name = match app_bundle.file_name().and_then(|s| s.to_str()) {
        Some(n) => n.to_string(),
        None => return,
    };
    let target = std::path::PathBuf::from("/Applications").join(&app_name);

    // Shell helper: pause briefly so this process exits cleanly first, kill
    // any pre-existing Niyora running from /Applications (so ditto can
    // overwrite), copy, then launch from the new location. If any of those
    // steps fail (locked /Applications on a managed Mac, disk full, etc.),
    // surface a native dialog with manual instructions rather than failing
    // silently — the user has just exited the source process and would
    // otherwise see nothing happen.
    let script = format!(
        r#"sleep 0.4
pkill -f "{target_str}/Contents/MacOS/" 2>/dev/null || true
sleep 0.2
if rm -rf "{target_str}" \
  && /usr/bin/ditto "{src_str}" "{target_str}" \
  && /usr/bin/open -n "{target_str}"; then
    exit 0
fi
/usr/bin/osascript -e 'display dialog "Niyora could not move itself to your Applications folder. Open Finder, drag Niyora.app from the Niyora disk to your Applications folder, then open it from there." with title "Niyora" buttons {{"OK"}} default button 1 with icon caution'
"#,
        target_str = target.to_string_lossy(),
        src_str = app_bundle.to_string_lossy(),
    );
    let _ = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    // Drop a marker so the relocated copy knows to show the welcome panel
    // even if the user has already onboarded. Without this, a re-install
    // looks like nothing happened — the tray icon was already there.
    if let Some(dir) = config::app_data_dir() {
        let _ = std::fs::write(dir.join(".show_panel_on_next_launch"), "1");
    }

    // Exit fast. The detached helper takes over from here.
    std::process::exit(0);
}

#[cfg(target_os = "macos")]
fn consume_show_panel_marker() -> bool {
    let Some(dir) = config::app_data_dir() else { return false; };
    let marker = dir.join(".show_panel_on_next_launch");
    if marker.exists() {
        let _ = std::fs::remove_file(&marker);
        true
    } else {
        false
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    relocate_from_dmg_if_needed();

    let last_session = Arc::new(Mutex::new(Instant::now()));
    let situational = Arc::new(Mutex::new(SituationalState::new()));
    let snoozed: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let tray_rect: Arc<Mutex<Option<(f64, f64, f64, f64)>>> = Arc::new(Mutex::new(None));
    let session_active: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let current_tray_state: Arc<Mutex<TrayState>> = Arc::new(Mutex::new(TrayState::Measuring));

    tauri::Builder::default()
        .manage(LastSessionTime(last_session.clone()))
        .manage(SituationalStateHandle(situational.clone()))
        .manage(SnoozedUntil(snoozed.clone()))
        .manage(TrayRect(tray_rect.clone()))
        .manage(SessionActive(session_active.clone()))
        .manage(CurrentTrayState(current_tray_state.clone()))
        .invoke_handler(tauri::generate_handler![
            hide_panel,
            resize_panel,
            set_session_active,
            situational::get_situational_snapshot,
            analytics::log_event,
            analytics::should_show_pss4,
            analytics::submit_pss4,
            analytics::dismiss_pss4,
            analytics::pss4_history,
            reminder::snooze_for_minutes,
            reminder::cancel_snooze,
            sessions::record_session,
            sessions::get_session_stats,
            onboarding::is_onboarded,
            onboarding::mark_onboarded,
            onboarding::request_notification_permission,
        ])
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_nspanel::init())
        .plugin({
            // Global hotkey: Cmd+Option+Shift+N toggles the panel. The tray
            // icon can disappear behind notches or get pushed off when the
            // menu bar is crowded; this gives users a guaranteed way in.
            use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
            let toggle_shortcut = Shortcut::new(
                Some(Modifiers::SUPER | Modifiers::ALT | Modifiers::SHIFT),
                Code::KeyN,
            );
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &toggle_shortcut && event.state() == ShortcutState::Pressed {
                        #[cfg(target_os = "macos")]
                        toggle_panel(app);
                    }
                })
                .build()
        })
        .setup(move |app| {
            // Hide from dock — menu bar only
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tell mac-notification-sys which app is sending notifications.
            // Without this, notifications appear under Finder's identity.
            #[cfg(target_os = "macos")]
            let _ = mac_notification_sys::set_application("com.niyora.breathing");

            // Convert the main window to an NSPanel for proper popover behavior
            #[cfg(target_os = "macos")]
            setup_panel(app.handle());

            // Build the tray icon. The body PNG (ring + nucleus only) is set
            // as a template so macOS auto-tints it per-pixel against the
            // local menu-bar background — same path the Wi-Fi, battery, and
            // clock icons take. The coloured electron is composited on top
            // by `tray_mac::apply_state` once the tray is up.
            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray-body.png"))
                .expect("failed to load tray icon");

            // Tray right-click menu. To add items: extend the MenuBuilder chain below
            // and add a matching arm to on_menu_event. Each item needs a unique id.
            //   "show"      -> Show Niyora (toggles the panel)
            //   "snooze_1h" -> Suppress reminders for the next 1 hour
            //   "quit"      -> Cleanly exit the app
            let show_item =
                MenuItemBuilder::with_id("show", "Show Niyora    ⌘⌥⇧N").build(app)?;
            let snooze_item =
                MenuItemBuilder::with_id("snooze_1h", "Snooze for 1 hour").build(app)?;
            let check_updates_item =
                MenuItemBuilder::with_id("check_updates", "Check for Updates…").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit Niyora").build(app)?;
            // Dev-only items: trigger a notification on demand and re-trigger
            // the onboarding flow. Both removed before App Store submission.
            #[cfg(debug_assertions)]
            let test_notif_item =
                MenuItemBuilder::with_id("test_notif", "Test notification").build(app)?;
            #[cfg(debug_assertions)]
            let reset_onboarding_item =
                MenuItemBuilder::with_id("reset_onboarding", "Reset onboarding (dev)").build(app)?;
            #[cfg(debug_assertions)]
            let reset_sessions_item =
                MenuItemBuilder::with_id("reset_sessions", "Reset sessions (dev)").build(app)?;
            let mut menu_builder = MenuBuilder::new(app)
                .item(&show_item)
                .item(&PredefinedMenuItem::separator(app)?)
                .item(&snooze_item);
            #[cfg(debug_assertions)]
            {
                menu_builder = menu_builder
                    .item(&PredefinedMenuItem::separator(app)?)
                    .item(&test_notif_item)
                    .item(&reset_onboarding_item)
                    .item(&reset_sessions_item);
            }
            let menu = menu_builder
                .item(&PredefinedMenuItem::separator(app)?)
                .item(&check_updates_item)
                .item(&PredefinedMenuItem::separator(app)?)
                .item(&quit_item)
                .build()?;

            TrayIconBuilder::with_id("tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Niyora · Breathe (⌘⌥⇧N)")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        #[cfg(target_os = "macos")]
                        toggle_panel(app);
                    }
                    "snooze_1h" => {
                        if let Some(state) = app.try_state::<SnoozedUntil>() {
                            let deadline = Instant::now() + Duration::from_secs(3600);
                            *state.0.lock().unwrap() = Some(deadline);
                        }
                        let _ = analytics::append_event(
                            "snooze_started",
                            serde_json::json!({ "duration_minutes": 60 }),
                        );
                        set_tray_state(app, TrayState::Measuring);
                    }
                    "check_updates" => {
                        updater::check(app.clone(), true);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    #[cfg(debug_assertions)]
                    "test_notif" => {
                        reminder::fire_test_notification(app.clone());
                    }
                    #[cfg(debug_assertions)]
                    "reset_onboarding" => {
                        let _ = onboarding::reset_onboarded();
                        // Force the panel to re-show so the user sees the
                        // onboarding flow on the very next interaction.
                        let _ = app.emit("onboarding_reset", ());
                    }
                    #[cfg(debug_assertions)]
                    "reset_sessions" => {
                        let _ = sessions::reset_sessions();
                        // Tell the frontend to refetch session stats so the
                        // next session is treated as the first (Box Breath
                        // default + AHA screen).
                        let _ = app.emit("sessions_reset", ());
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

                    // Capture tray icon rect on every event so toggle_panel
                    // always has a fresh anchor point.
                    if let Some(rect) = tray_rect_from_event(&event) {
                        if let Some(state) = tray.app_handle().try_state::<TrayRect>() {
                            *state.0.lock().unwrap() = Some(rect);
                        }
                    }

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

            // Register the global toggle shortcut. Same key combo as the
            // plugin handler above (Cmd+Option+Shift+N). Failures are logged
            // but non-fatal: the tray icon and notifications still work
            // without a hotkey.
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let toggle_shortcut = Shortcut::new(
                    Some(Modifiers::SUPER | Modifiers::ALT | Modifiers::SHIFT),
                    Code::KeyN,
                );
                if let Err(e) = app.global_shortcut().register(toggle_shortcut) {
                    eprintln!("global-shortcut register failed: {e}");
                }
            }

            // First-launch auto-open: if the user hasn't onboarded, surface
            // the panel a moment after launch so they don't have to hunt
            // for the menu-bar icon. The panel anchors under the tray
            // icon when its rect is known, otherwise toggle_panel falls
            // back to a top-right position near the menu bar so the visual
            // connection still reads. Onboarding adds an explicit "I live
            // up here" pointer.
            // Auto-show the panel if the user hasn't onboarded, or if the app
            // was just relocated from a DMG. The latter gives a clear visual
            // confirmation that the install/re-install actually worked.
            #[cfg(target_os = "macos")]
            let just_relocated = consume_show_panel_marker();
            #[cfg(not(target_os = "macos"))]
            let just_relocated = false;
            let needs_onboarding = !onboarding::is_onboarded();
            if needs_onboarding || just_relocated {
                let handle_for_first_launch = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(900));
                    let h = handle_for_first_launch.clone();
                    let _ = handle_for_first_launch.run_on_main_thread(move || {
                        #[cfg(target_os = "macos")]
                        toggle_panel(&h);
                    });
                    // Spin the tray electron for a few seconds so a first-time
                    // user's eye lands on the menu-bar icon, not just the
                    // panel. Skipped on plain DMG re-installs (the panel is
                    // pop-up enough on its own).
                    #[cfg(target_os = "macos")]
                    if needs_onboarding {
                        tray_mac::run_attention_animation(handle_for_first_launch);
                    }
                });
            }

            // Listen for "open the panel from a notification" requests.
            // Fired by the notification action handler in reminder.rs.
            let app_handle_for_listen = app.handle().clone();
            app.handle().listen("request_panel_show", move |_| {
                let h = app_handle_for_listen.clone();
                let _ = app_handle_for_listen.run_on_main_thread(move || {
                    #[cfg(target_os = "macos")]
                    {
                        let panel = h.get_webview_panel("main").unwrap();
                        if !panel.is_visible() {
                            toggle_panel(&h);
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    let _ = h;
                });
            });

            // Background update check (privacy-disclosed; see updater.rs).
            // Delayed a few seconds so it never blocks tray-icon registration
            // or first paint of the panel.
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(5));
                    updater::maybe_silent_check(handle);
                });
            }

            // Composite the coloured electron over the template body. We
            // wait briefly so the NSStatusItem is fully wired up — the
            // tray builder finishes synchronously above, but the underlying
            // status-item button isn't always immediately discoverable via
            // `_statusItems`. Re-apply on every state change happens via
            // `set_tray_state` from then on.
            #[cfg(target_os = "macos")]
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(500));
                    let _ = app_handle.run_on_main_thread(|| {
                        tray_mac::apply_state(TrayState::Measuring, None);
                    });
                });
            }

            // Start situational signal collectors (idle/screen-time + score loop)
            situational::start_all(situational.clone());

            // Smart screen-time reminder thread
            let app_handle = app.handle().clone();
            let last_session_for_thread = last_session.clone();
            let situational_for_thread = situational.clone();
            let snoozed_for_thread = snoozed.clone();
            std::thread::spawn(move || {
                reminder::run(
                    app_handle,
                    last_session_for_thread,
                    situational_for_thread,
                    snoozed_for_thread,
                );
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Niyora")
        .run(|app_handle, event| {
            // When the user clicks Niyora.app from Finder or Spotlight while
            // it is already running, macOS sends a "reopen" event. This is
            // the recovery path for users who lost the tray icon to a full
            // menu bar / notch overflow. We pop the panel so they have an
            // immediate way back in, and toggle the tray's visibility so
            // macOS gets a chance to re-place it where it can be seen.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(tray) = app_handle.tray_by_id("tray") {
                    let _ = tray.set_visible(false);
                    let _ = tray.set_visible(true);
                }
                let panel = app_handle.get_webview_panel("main").unwrap();
                if !panel.is_visible() {
                    toggle_panel(app_handle);
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = app_handle;
                let _ = event;
            }
        });
}

/// Convert the main window into an NSPanel with proper menubar popover behavior.
#[cfg(target_os = "macos")]
fn setup_panel(app_handle: &tauri::AppHandle) {
    let window = app_handle
        .get_webview_window("main")
        .expect("no window labeled 'main'");

    let delegate = panel_delegate!(NiyoraPanelDelegate {
        window_did_resign_key
    });

    let handle = app_handle.clone();
    delegate.set_listener(Box::new(move |event_name: String| {
        if event_name.as_str() == "window_did_resign_key" {
            let _ = handle.emit("panel_did_resign_key", ());
        }
    }));

    let panel = window.to_panel().unwrap();

    panel.set_level(NSMainMenuWindowLevel + 1);

    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );

    panel.set_style_mask(NSPANEL_STYLE_MASK_NON_ACTIVATING);

    panel.set_delegate(delegate);

    let handle = app_handle.clone();
    app_handle.listen("panel_did_resign_key", move |_| {
        // Stay sticky during onboarding (so a stray click can't lose the
        // welcome screen) and during an active breathing session (so a
        // stray click can't abort a 60-second practice). Otherwise honor
        // the standard menu-bar app pattern of click-outside-to-dismiss.
        if !onboarding::is_onboarded() {
            return;
        }
        let active = handle
            .try_state::<SessionActive>()
            .map(|s| *s.0.lock().unwrap())
            .unwrap_or(false);
        if active {
            return;
        }
        let panel = handle.get_webview_panel("main").unwrap();
        if panel.is_visible() {
            panel.order_out(None);
        }
    });
}

/// Extract the tray icon's position+size from any TrayIconEvent variant
/// that carries a `rect`. Returns None for events without one.
/// All coordinates are in physical pixels.
fn tray_rect_from_event(event: &TrayIconEvent) -> Option<(f64, f64, f64, f64)> {
    let rect = match event {
        TrayIconEvent::Click { rect, .. } => rect,
        TrayIconEvent::DoubleClick { rect, .. } => rect,
        TrayIconEvent::Enter { rect, .. } => rect,
        TrayIconEvent::Move { rect, .. } => rect,
        TrayIconEvent::Leave { rect, .. } => rect,
        _ => return None,
    };
    let pos = rect.position.to_physical::<f64>(1.0);
    let size = rect.size.to_physical::<f64>(1.0);
    Some((pos.x, pos.y, size.width, size.height))
}

/// Toggle the panel: show anchored directly under the tray icon, or hide.
/// Anchored using the captured tray rect — deterministic, no positioner
/// race conditions, no off-screen clipping at the edges.
#[cfg(target_os = "macos")]
fn toggle_panel(app_handle: &tauri::AppHandle) {
    let panel = app_handle.get_webview_panel("main").unwrap();

    if panel.is_visible() {
        panel.order_out(None);
        return;
    }

    if let Some(window) = app_handle.get_webview_window("main") {
        let tray = app_handle
            .try_state::<TrayRect>()
            .and_then(|s| *s.0.lock().unwrap());

        if let Ok(Some(monitor)) = window.current_monitor() {
            let screen = monitor.size();
            let scale = monitor.scale_factor();
            let panel_w_logical: f64 = 420.0;
            let panel_w_phys = panel_w_logical * scale;
            let screen_w = screen.width as f64;
            let edge_margin = 8.0 * scale;

            // Center panel under tray icon when its rect is known, otherwise
            // fall back to top-right of the screen near where macOS places
            // app tray icons. Used on first launch before any tray event
            // has been dispatched.
            let (mut x_phys, y_phys) = if let Some((tx, _ty, tw, th)) = tray {
                let tray_center_x = tx + tw / 2.0;
                let x = tray_center_x - panel_w_phys / 2.0;
                let gap_phys = 4.0 * scale;
                (x, th + gap_phys)
            } else {
                let menu_bar_phys = 24.0 * scale;
                let gap_phys = 4.0 * scale;
                let x = screen_w - panel_w_phys - edge_margin;
                (x, menu_bar_phys + gap_phys)
            };

            if x_phys + panel_w_phys + edge_margin > screen_w {
                x_phys = screen_w - panel_w_phys - edge_margin;
            }
            if x_phys < edge_margin {
                x_phys = edge_margin;
            }

            let _ = window.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition {
                    x: x_phys.round() as i32,
                    y: y_phys.round() as i32,
                },
            ));
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    panel.show();

    // Any path that opens the panel counts as the user acknowledging an
    // outstanding reminder, so revert the tray icon to Measuring. Cheap
    // no-op if it was already Measuring.
    set_tray_state(app_handle, TrayState::Measuring);

    // Tell the React app to reset its session state for this open.
    // Replaces the previous full-page reload, which caused a visible reflow
    // on every tray click.
    let _ = app_handle.emit("panel_did_show", ());

    // Catches users who keep the tray app running for days/weeks; the launch
    // check alone never fires for them. Internally throttled to 24h, so this
    // is a cheap no-op on rapid panel toggles.
    updater::maybe_silent_check(app_handle.clone());
}

#[cfg(not(target_os = "macos"))]
fn toggle_panel(_app_handle: &tauri::AppHandle) {}
