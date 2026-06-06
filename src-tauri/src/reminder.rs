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

/// Timestamp of the most recent session recorded by the paired iPhone.
/// `None` until the first `session_recorded` frame arrives from a companion.
/// Read by the reminder loop to implement the active-device tie-break (spec
/// section 7): if the phone had a session more recently than the Mac, the
/// phone is the active device and the Mac stays silent.
pub struct LastPhoneSessionTime(pub Arc<Mutex<Option<Instant>>>);

const CHECK_INTERVAL: Duration = Duration::from_secs(60);
const WORK_HOUR_START: u32 = 9;
const WORK_HOUR_END: u32 = 18;

/// Rotating notification copy. Picked pseudo-randomly per fire so the
/// nudge feels like a kind friend, not a robot. Keep these short: macOS
/// truncates notification bodies around 90-100 chars.
///
/// At least 4 entries cite a specific mechanism, timeframe, or outcome so
/// the nudge teaches as well as prompts. The rest keep a softer tone for
/// variety. All strings must stay under 90 chars (enforced by a test below).
const BODY_VARIANTS: &[&str] = &[
    "Slow breathing lowers cortisol within 90 seconds. Worth one minute.",
    "Six slow breaths activate the parasympathetic system in under 60s.",
    "Diaphragm breathing engages the vagus nerve, calming your heart rate.",
    "Box breathing cuts perceived stress in under two minutes.",
    "HRV rises measurably after 60 seconds of paced breathing.",
    "A slow exhale triggers the vagal brake, dropping heart rate quickly.",
    "A lot on the screen. A breath might help.",
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

/// Returns true if the Mac should fire the reminder, false if it should defer
/// to the paired phone.
///
/// Called only after the shared cooldown has expired. Rule: the device with
/// the most recent session activity owns the notification; phone wins ties
/// (spec section 7). When no phone session has been recorded in this app
/// session (`phone_session_elapsed` is `None`), the Mac fires normally.
pub(crate) fn mac_should_fire(
    phone_session_elapsed: Option<Duration>,
    mac_session_elapsed: Duration,
) -> bool {
    match phone_session_elapsed {
        Some(phone_elapsed) => phone_elapsed > mac_session_elapsed,
        None => true,
    }
}

/// Smart screen-time reminder: notifies after the dynamic threshold elapses
/// (40-120 min depending on situational signals), during work hours.
/// Notifications carry inline action buttons:
///   "Breathe now"   → opens the panel (emits `request_panel_show`)
///   "Snooze 30 min" → sets the snooze deadline directly
/// Clicking the notification body itself also opens the panel.
///
/// When a phone is paired, `last_phone_session` carries the timestamp of the
/// most recent session the phone reported via `session_recorded`. The reminder
/// loop uses it to implement the active-device rule: if the phone was active
/// more recently than the Mac, the Mac defers (spec section 7).
pub fn run(
    app: tauri::AppHandle,
    last_session: Arc<Mutex<Instant>>,
    situational: Arc<Mutex<SituationalState>>,
    snoozed_until: Arc<Mutex<Option<Instant>>>,
    last_phone_session: Arc<Mutex<Option<Instant>>>,
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

        // Active-device rule: when paired, defer to the phone if it had a
        // session more recently than the Mac. Phone wins ties (spec section 7).
        let phone_elapsed = last_phone_session.lock().unwrap().map(|t| t.elapsed());
        if !mac_should_fire(phone_elapsed, elapsed) {
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

/// Show a reminder notification. macOS uses mac-notification-sys directly so
/// the banner carries inline action buttons ("Breathe now" / "Snooze 30 min");
/// Windows uses a plain toast (no inline buttons, see project memory).
///
/// macOS spawns a worker thread to wait for the user's response so the reminder
/// loop is never blocked. Windows fires-and-forgets; the amber tray icon stays
/// up until the user opens the panel.
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
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("Niyora")
                .body(&body)
                .show();
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

#[cfg(test)]
mod tests {
    use super::{mac_should_fire, BODY_VARIANTS};
    use std::time::Duration;

    #[test]
    fn body_variants_length() {
        assert!(
            BODY_VARIANTS.len() >= 10,
            "BODY_VARIANTS must have at least 10 entries"
        );
        for s in BODY_VARIANTS {
            assert!(
                s.len() <= 90,
                "notification body exceeds 90 chars ({} chars): {:?}",
                s.len(),
                s
            );
        }
    }

    // Active-device / shared-cooldown decision tests (spec section 7).

    #[test]
    fn no_phone_session_mac_fires() {
        assert!(mac_should_fire(None, Duration::from_secs(3600)));
    }

    #[test]
    fn phone_more_recent_mac_suppressed() {
        // Phone session 30 min ago, Mac session 60 min ago.
        assert!(!mac_should_fire(
            Some(Duration::from_secs(30 * 60)),
            Duration::from_secs(60 * 60),
        ));
    }

    #[test]
    fn mac_more_recent_mac_fires() {
        // Mac session 30 min ago, phone session 60 min ago.
        assert!(mac_should_fire(
            Some(Duration::from_secs(60 * 60)),
            Duration::from_secs(30 * 60),
        ));
    }

    #[test]
    fn equal_age_phone_wins_tiebreak() {
        let age = Duration::from_secs(90 * 60);
        assert!(!mac_should_fire(Some(age), age));
    }
}
