use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};

use super::{score, SituationalState};

const IDLE_BREAK_THRESHOLD_SECS: u64 = 5 * 60; // 5 min idle = break
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(30);
const SCORE_POLL_INTERVAL: Duration = Duration::from_secs(60);

const MIC_POLL_INTERVAL: Duration = Duration::from_secs(15);
const MIC_CONFIRM_TICKS: u32 = 2; // 2 consecutive ticks (~30s) before transitioning
const BACK_TO_BACK_GAP_SECS: u64 = 10 * 60; // <10 min gap = back-to-back

const APP_POLL_INTERVAL: Duration = Duration::from_secs(30);
const APP_SWITCH_WINDOW_SECS: u64 = 30 * 60; // rolling 30-min window

const INPUT_POLL_INTERVAL: Duration = Duration::from_secs(60); // diff over 1 min = events/min

pub fn start_all(
    state: Arc<Mutex<SituationalState>>,
    on_soul_state_change: Option<Arc<dyn Fn(String, u8) + Send + Sync + 'static>>,
) {
    // Immediate first compute so any score/message overrides + the initial
    // signal state surface right away — without waiting 60s for the score loop.
    {
        let mut s = state.lock().unwrap();
        score::recompute(&mut s);
    }

    let s1 = state.clone();
    thread::spawn(move || idle_loop(s1));

    let s2 = state.clone();
    thread::spawn(move || meeting_loop(s2));

    let s3 = state.clone();
    thread::spawn(move || app_loop(s3));

    let s4 = state.clone();
    thread::spawn(move || input_loop(s4));

    let s5 = state;
    thread::spawn(move || score_loop(s5, on_soul_state_change));
}

/// Tracks continuous screen time. Polls system idle every 30s.
/// 5+ minutes of idle = a break ended; reset the streak when activity resumes.
fn idle_loop(state: Arc<Mutex<SituationalState>>) {
    let mut last_break_at = Instant::now();
    let mut was_idle = false;

    loop {
        thread::sleep(IDLE_POLL_INTERVAL);

        let idle_secs = system_idle_seconds() as u64;
        let is_idle = idle_secs >= IDLE_BREAK_THRESHOLD_SECS;

        // Transition from idle → active marks the end of a break
        if was_idle && !is_idle {
            last_break_at = Instant::now();
        }
        was_idle = is_idle;

        let continuous = if is_idle {
            0
        } else {
            last_break_at.elapsed().as_secs() / 60
        };

        let mut s = state.lock().unwrap();
        s.last_break_at = last_break_at;
        s.continuous_screen_min = continuous;
    }
}

/// Detects "in a meeting" by polling whether the default audio input device
/// is currently running (i.e., some app is recording from the mic).
/// Works across Zoom, Teams, Meet, Slack, FaceTime — any app that opens the mic.
/// Requires no permissions: querying device-running state is not the same as recording.
fn meeting_loop(state: Arc<Mutex<SituationalState>>) {
    let mut in_meeting = false;
    let mut meeting_started_at: Option<Instant> = None;
    let mut last_meeting_ended_at: Option<Instant> = None;
    let mut hot_streak: u32 = 0;
    let mut quiet_streak: u32 = 0;

    loop {
        thread::sleep(MIC_POLL_INTERVAL);

        let hot = mic_is_running();
        if hot {
            hot_streak += 1;
            quiet_streak = 0;
        } else {
            quiet_streak += 1;
            hot_streak = 0;
        }

        // Confirmed meeting start
        if !in_meeting && hot_streak >= MIC_CONFIRM_TICKS {
            in_meeting = true;
            meeting_started_at = Some(Instant::now());

            let bb = matches!(
                last_meeting_ended_at,
                Some(end) if end.elapsed() < Duration::from_secs(BACK_TO_BACK_GAP_SECS)
            );

            let mut s = state.lock().unwrap();
            s.in_meeting = true;
            if bb {
                s.back_to_back_count = s.back_to_back_count.saturating_add(1);
            } else {
                s.back_to_back_count = 0;
            }
        }

        // Confirmed meeting end
        if in_meeting && quiet_streak >= MIC_CONFIRM_TICKS {
            in_meeting = false;
            let end_time = Instant::now();
            last_meeting_ended_at = Some(end_time);

            if let Some(start) = meeting_started_at {
                let duration_min = end_time.saturating_duration_since(start).as_secs() / 60;
                let mut s = state.lock().unwrap();
                s.cumulative_meeting_min_today =
                    s.cumulative_meeting_min_today.saturating_add(duration_min);
                s.current_meeting_min = 0;
                s.in_meeting = false;
                s.last_meeting_ended_at = Some(end_time);
            }
            meeting_started_at = None;
        }

        // Live update of current meeting duration while active
        if in_meeting {
            if let Some(start) = meeting_started_at {
                let mins = start.elapsed().as_secs() / 60;
                state.lock().unwrap().current_meeting_min = mins;
            }
        }
    }
}

/// Tracks the frontmost application and counts app switches in a rolling 30-min window.
/// Also measures `focus_block_min` — uninterrupted time on the same app.
fn app_loop(state: Arc<Mutex<SituationalState>>) {
    let mut last_seen: Option<String> = None;
    let mut focus_started_at = Instant::now();
    let mut switch_times: VecDeque<Instant> = VecDeque::new();

    loop {
        thread::sleep(APP_POLL_INTERVAL);
        let now = Instant::now();

        // Expire switch timestamps older than the rolling window
        while let Some(&front) = switch_times.front() {
            if now.duration_since(front).as_secs() > APP_SWITCH_WINDOW_SECS {
                switch_times.pop_front();
            } else {
                break;
            }
        }

        let current = frontmost_bundle_id();

        // Only count Some→Some transitions where the bundle changed.
        // Some→None and None→Some are treated as no-ops to avoid spurious switches
        // when a brief transient leaves no frontmost app.
        if let Some(ref c) = current {
            match &last_seen {
                Some(prev) if prev != c => {
                    switch_times.push_back(now);
                    focus_started_at = now;
                }
                None => {
                    focus_started_at = now;
                }
                _ => {}
            }
            last_seen = Some(c.clone());
        }

        let focus_min = if last_seen.is_some() {
            focus_started_at.elapsed().as_secs() / 60
        } else {
            0
        };

        let mut s = state.lock().unwrap();
        s.current_app_bundle = current;
        s.app_switches_last_30min = switch_times.len() as u32;
        s.focus_block_min = focus_min;
    }
}

/// Polls the system-wide cumulative event counters and computes per-minute rates
/// by diffing across polls. Permission-free: this is an aggregate counter,
/// not per-keystroke logging — no key contents are visible.
fn input_loop(state: Arc<Mutex<SituationalState>>) {
    let mut prev_keys: Option<u32> = None;
    let mut prev_clicks: Option<u32> = None;

    loop {
        thread::sleep(INPUT_POLL_INTERVAL);

        let now_keys = event_count(EVENT_TYPE_KEY_DOWN);
        let now_clicks = event_count(EVENT_TYPE_LEFT_MOUSE_DOWN);

        let key_rate = match prev_keys {
            Some(prev) => now_keys.wrapping_sub(prev),
            None => 0,
        };
        let click_rate = match prev_clicks {
            Some(prev) => now_clicks.wrapping_sub(prev),
            None => 0,
        };

        prev_keys = Some(now_keys);
        prev_clicks = Some(now_clicks);

        let mut s = state.lock().unwrap();
        s.keystrokes_per_min = key_rate;
        s.clicks_per_min = click_rate;
    }
}

/// Recomputes Niyora Index, base interval, day label, and contextual message
/// once a minute. Also handles daily reset of cumulative counters.
/// Calls `on_soul_state_change` when the day label transitions so paired
/// phones receive an updated `soul_state` frame without waiting for a reconnect.
fn score_loop(
    state: Arc<Mutex<SituationalState>>,
    on_soul_state_change: Option<Arc<dyn Fn(String, u8) + Send + Sync + 'static>>,
) {
    let mut prev_label: &'static str = "";

    loop {
        thread::sleep(SCORE_POLL_INTERVAL);

        let now = chrono::Local::now();
        let today = now.date_naive();
        let hour = now.hour();
        let weekday = now.weekday().num_days_from_monday();
        let after_hours = !(9..18).contains(&hour) || weekday >= 5;

        let (new_label, new_index) = {
            let mut s = state.lock().unwrap();

            if today != s.last_reset_date {
                s.cumulative_meeting_min_today = 0;
                s.back_to_back_count = 0;
                s.last_reset_date = today;
            }

            s.after_hours = after_hours;
            score::recompute(&mut s);
            (s.day_label.as_str(), s.niyora_index)
        };

        if new_label != prev_label {
            prev_label = new_label;
            if let Some(ref cb) = on_soul_state_change {
                cb(new_label.to_string(), new_index);
            }
        }
    }
}

// --- macOS system idle bridge ---

#[cfg(target_os = "macos")]
const K_CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE: u32 = 1;
#[cfg(target_os = "macos")]
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

// CGEventType values (from CGEventTypes.h) — referenced from cross-platform code
const EVENT_TYPE_LEFT_MOUSE_DOWN: u32 = 1;
const EVENT_TYPE_KEY_DOWN: u32 = 10;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(
        source_state_id: u32,
        event_type: u32,
    ) -> f64;

    fn CGEventSourceCounterForEventType(
        source_state_id: u32,
        event_type: u32,
    ) -> u32;
}

#[cfg(target_os = "macos")]
fn system_idle_seconds() -> f64 {
    unsafe {
        CGEventSourceSecondsSinceLastEventType(
            K_CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE,
            K_CG_ANY_INPUT_EVENT_TYPE,
        )
    }
}

#[cfg(target_os = "windows")]
fn system_idle_seconds() -> f64 {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    let ok = unsafe { GetLastInputInfo(&mut lii) };
    if ok == 0 {
        return 0.0;
    }
    // GetTickCount wraps every 49.7 days. wrapping_sub keeps idle correct
    // across that boundary since dwTime and GetTickCount share the same
    // 32-bit modulus.
    let idle_ms = unsafe { GetTickCount() }.wrapping_sub(lii.dwTime);
    (idle_ms as f64) / 1000.0
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn system_idle_seconds() -> f64 {
    0.0
}

#[cfg(target_os = "macos")]
fn event_count(event_type: u32) -> u32 {
    unsafe {
        CGEventSourceCounterForEventType(
            K_CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE,
            event_type,
        )
    }
}

#[cfg(not(target_os = "macos"))]
fn event_count(_event_type: u32) -> u32 {
    0
}

// --- macOS CoreAudio bridge: is the default input device currently running? ---

#[cfg(target_os = "macos")]
#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[cfg(target_os = "macos")]
const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
#[cfg(target_os = "macos")]
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 0x676c6f62; // 'glob'
#[cfg(target_os = "macos")]
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;
#[cfg(target_os = "macos")]
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE: u32 = 0x64496e20; // 'dIn '
#[cfg(target_os = "macos")]
const K_AUDIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE: u32 = 0x676f6e65; // 'gone'

#[cfg(target_os = "macos")]
#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyData(
        in_object_id: u32,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const std::ffi::c_void,
        io_data_size: *mut u32,
        out_data: *mut std::ffi::c_void,
    ) -> i32;
}

#[cfg(target_os = "macos")]
fn default_input_device_id() -> Option<u32> {
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut device_id: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut device_id as *mut u32 as *mut _,
        )
    };
    if status == 0 && device_id != 0 {
        Some(device_id)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn mic_is_running() -> bool {
    let Some(device_id) = default_input_device_id() else { return false; };
    let addr = AudioObjectPropertyAddress {
        selector: K_AUDIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE,
        scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };
    let mut running: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            &mut running as *mut u32 as *mut _,
        )
    };
    status == 0 && running != 0
}

#[cfg(not(target_os = "macos"))]
fn mic_is_running() -> bool {
    false
}

// --- macOS NSWorkspace bridge: bundle identifier of the frontmost application ---

#[cfg(target_os = "macos")]
fn frontmost_bundle_id() -> Option<String> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        if pool.is_null() {
            return None;
        }

        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            let _: () = msg_send![pool, drain];
            return None;
        }

        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            let _: () = msg_send![pool, drain];
            return None;
        }

        let bundle_id: *mut Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() {
            let _: () = msg_send![pool, drain];
            return None;
        }

        let utf8: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if utf8.is_null() {
            let _: () = msg_send![pool, drain];
            return None;
        }

        let result = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
        let _: () = msg_send![pool, drain];
        Some(result)
    }
}

#[cfg(target_os = "windows")]
fn frontmost_bundle_id() -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }

        let h_proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h_proc.is_null() {
            return None;
        }

        let mut buf: [u16; 260] = [0; 260]; // MAX_PATH
        let mut size: u32 = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h_proc, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(h_proc);

        if ok == 0 || size == 0 {
            return None;
        }

        let path = String::from_utf16_lossy(&buf[..size as usize]);
        // Return just the executable name (e.g., "firefox.exe") to keep the
        // identifier short and avoid leaking user-specific install paths.
        std::path::Path::new(&path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .or(Some(path))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn frontmost_bundle_id() -> Option<String> {
    None
}
