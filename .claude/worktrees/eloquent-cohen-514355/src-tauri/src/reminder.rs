use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Timelike;
use serde_json::json;
use tauri::{Emitter, Manager};

use crate::analytics;
use crate::situational::SituationalState;

/// Shared timestamp of the last completed breathing session (or app launch).
/// Reset when user completes a session; checked by the reminder loop.
pub struct LastSessionTime(pub Arc<Mutex<Instant>>);

/// Deadline (Instant) until which screen-time reminders are suppressed.
/// `None` = not snoozed. `Some(t)` and `Instant::now() < t` = currently snoozed.
pub struct SnoozedUntil(pub Arc<Mutex<Option<Instant>>>);

const CHECK_INTERVAL: Duration = Duration::from_secs(60);
const WORK_HOUR_START: u32 = 9;
const WORK_HOUR_END: u32 = 18;

/// Rotating notification copy. Picked pseudo-randomly per fire so the
/// nudge feels like a kind friend, not a robot. Keep these short — most
/// users only read the first few words on macOS.
const BODY_VARIANTS: &[&str] = &[
    "A lot on the screen. A breath might help.",
    "Lots of switching today. Come back to one place.",
    "Body's been working hard. Give it a minute?",
    "We had a deal. Sixty seconds?",
    "You'll think clearer after a reset.",
];

fn pick_body() -> &'static str {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    BODY_VARIANTS[nanos % BODY_VARIANTS.len()]
}

/// Smart screen-time reminder: notifies after the dynamic threshold elapses
/// (40-120 min depending on situational signals), during work hours.
/// Notifications carry inline action buttons:
///   "Breathe now"   → opens the panel (emits `request_panel_show`)
///   "Snooze 30 min" → sets the snooze deadline directly
/// Clicking the notification body itself also opens the panel.
pub fn run(
    app: tauri::AppHandle,
    last_session: Arc<Mutex<Instant>>,
    situational: Arc<Mutex<SituationalState>>,
    snoozed_until: Arc<Mutex<Option<Instant>>>,
) {
    use chrono::Local;
    use std::thread::sleep;

    let mut notified_for_current_period = false;

    loop {
        sleep(CHECK_INTERVAL);

        let hour = Local::now().hour();
        if !(WORK_HOUR_START..WORK_HOUR_END).contains(&hour) {
            continue;
        }

        // Snooze check: if a deadline is set and still in the future, skip this tick.
        // Once expired, clear it so future ticks proceed normally.
        {
            let mut guard = snoozed_until.lock().unwrap();
            if let Some(deadline) = *guard {
                if Instant::now() < deadline {
                    continue;
                }
                *guard = None;
            }
        }

        let (interval_min, score, label) = {
            let s = situational.lock().unwrap();
            (
                s.current_interval_min,
                s.niyora_index,
                s.day_label.as_str().to_string(),
            )
        };
        let limit = Duration::from_secs(interval_min * 60);

        let elapsed = last_session.lock().unwrap().elapsed();

        if elapsed < limit {
            notified_for_current_period = false;
            continue;
        }

        if !notified_for_current_period {
            fire_notification(app.clone());

            let _ = analytics::append_event(
                "reminder_fired",
                json!({
                    "interval_min": interval_min,
                    "score": score,
                    "label": label,
                    "snoozed": false,
                }),
            );

            notified_for_current_period = true;
        }
    }
}

/// Dev-only: fire a notification immediately, bypassing the screen-time
/// timer. Wired to the "Test notification" tray menu item.
#[cfg(debug_assertions)]
pub fn fire_test_notification(app: tauri::AppHandle) {
    fire_notification(app);
}

/// Show a macOS notification with two action buttons. Spawns a worker thread
/// to wait for the user's interaction so this function returns immediately
/// and the reminder loop is never blocked.
fn fire_notification(app: tauri::AppHandle) {
    let body = pick_body().to_string();

    // Switch the menu-bar icon to the amber-electron "Reminder" state so
    // the tray gives the same nudge as the notification, even if the user
    // dismisses the banner before reading it.
    crate::set_tray_state(&app, crate::TrayState::Reminder);

    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            use mac_notification_sys::{MainButton, Notification, NotificationResponse};

            let response = Notification::new()
                .title("Niyora")
                .message(&body)
                .main_button(MainButton::SingleAction("Breathe now"))
                .close_button("Snooze 30 min")
                .send();

            let response = match response {
                Ok(r) => r,
                Err(_) => {
                    crate::set_tray_state(&app, crate::TrayState::Measuring);
                    return;
                }
            };

            match response {
                NotificationResponse::ActionButton(_) | NotificationResponse::Click => {
                    request_panel_show(&app);
                }
                NotificationResponse::CloseButton(_) => {
                    apply_snooze_30min(&app);
                }
                _ => {}
            }

            // Any response (action, snooze, dismiss) means the user has
            // seen the nudge — drop the amber tray back to green.
            crate::set_tray_state(&app, crate::TrayState::Measuring);
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, body);
        }
    });
}

#[cfg(target_os = "macos")]
fn request_panel_show(app: &tauri::AppHandle) {
    let h = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = h.emit("request_panel_show", ());
    });
}

#[cfg(target_os = "macos")]
fn apply_snooze_30min(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<SnoozedUntil>() {
        let deadline = Instant::now() + Duration::from_secs(30 * 60);
        *state.0.lock().unwrap() = Some(deadline);
    }
    let _ = analytics::append_event(
        "snooze_started",
        json!({ "duration_minutes": 30, "source": "notification" }),
    );
}

/// Snooze reminders for the given number of minutes.
/// Frontend-callable; menu uses the same SnoozedUntil state directly.
#[tauri::command]
pub fn snooze_for_minutes(
    app: tauri::AppHandle,
    minutes: u64,
    state: tauri::State<'_, SnoozedUntil>,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(minutes * 60);
    *state.0.lock().unwrap() = Some(deadline);
    let _ = crate::analytics::append_event(
        "snooze_started",
        json!({ "duration_minutes": minutes }),
    );
    crate::set_tray_state(&app, crate::TrayState::Measuring);
    Ok(())
}

/// Clear any active snooze.
#[tauri::command]
pub fn cancel_snooze(state: tauri::State<'_, SnoozedUntil>) -> Result<(), String> {
    *state.0.lock().unwrap() = None;
    Ok(())
}
