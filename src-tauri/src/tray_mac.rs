// Hybrid tray icon: Tauri owns the NSStatusItem, the menu, and click
// handling; we reach in via the private `_statusItems` selector to add an
// NSImageView subview on top of the button. The button's image is the
// monochrome body (rendered as a template, so macOS auto-tints it per-pixel
// against the menu-bar background — same path Wi-Fi, battery, and clock
// icons take). The subview carries the coloured electron, drawn as-is so
// the green/amber state colour survives compositing.
//
// `_statusItems` has been stable since macOS 10.12. Niyora ships outside
// the App Store (open-source DMG), so the private-API caveat is fine here;
// inside our own process, it only returns status items we created.

#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSRect, NSSize, NSString};
use objc::runtime::{Sel, BOOL};
use objc::{class, msg_send, sel, sel_impl};

use crate::TrayState;

/// Tag we attach to our electron NSImageView so subsequent state updates
/// can find and replace it without leaking subviews on every transition.
const ELECTRON_TAG: i64 = 0x4E49594F; // "NIYO" in ASCII, just a sentinel

/// Logical size of the menu-bar icon (points). 1x = 22 px, 2x = 44 px.
const ICON_PT: f64 = 22.0;

/// Walk this app's NSStatusItems and return the one whose button tooltip
/// starts with "Niyora". Tauri sets that tooltip when building the tray,
/// so it's a reliable identifier without depending on iteration order.
unsafe fn find_status_item() -> Option<id> {
    let bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
    if bar == nil {
        return None;
    }
    let sel_items: Sel = sel!(_statusItems);
    let responds: BOOL = msg_send![bar, respondsToSelector: sel_items];
    if responds == NO {
        return None;
    }
    let items: id = msg_send![bar, _statusItems];
    if items == nil {
        return None;
    }
    // `_statusItems` returns an NSPointerArray (class
    // `NSConcretePointerArray`) — not an NSArray, so neither
    // `objectAtIndex:` nor `objectEnumerator` are valid on it. The one
    // selector that always works is `allObjects`, which returns a real
    // NSArray of the live (non-nil) entries.
    let array: id = msg_send![items, allObjects];
    if array == nil {
        return None;
    }
    let count: u64 = msg_send![array, count];
    if count == 0 {
        return None;
    }
    let needle = NSString::alloc(nil).init_str("Niyora");
    for i in 0..count {
        let item: id = msg_send![array, objectAtIndex: i];
        if item == nil {
            continue;
        }
        let button: id = msg_send![item, button];
        if button == nil {
            continue;
        }
        let tip: id = msg_send![button, toolTip];
        if tip == nil {
            continue;
        }
        let starts: BOOL = msg_send![tip, hasPrefix: needle];
        if starts == YES {
            return Some(item);
        }
    }
    None
}

/// Build a multi-resolution NSImage from 1x and 2x PNG bytes so retina
/// displays render the icon crisply without any extra scaling logic.
/// Returns `nil` if either PNG fails to decode — the caller should bail.
unsafe fn make_multires_nsimage(bytes_1x: &[u8], bytes_2x: &[u8]) -> id {
    let cls_data = class!(NSData);
    let data_1x: id = msg_send![cls_data,
        dataWithBytes: bytes_1x.as_ptr() as *const std::ffi::c_void
        length: bytes_1x.len() as u64];
    let data_2x: id = msg_send![cls_data,
        dataWithBytes: bytes_2x.as_ptr() as *const std::ffi::c_void
        length: bytes_2x.len() as u64];
    if data_1x == nil || data_2x == nil {
        return nil;
    }

    let cls_rep = class!(NSBitmapImageRep);
    let rep_1x: id = msg_send![cls_rep, imageRepWithData: data_1x];
    let rep_2x: id = msg_send![cls_rep, imageRepWithData: data_2x];
    if rep_1x == nil || rep_2x == nil {
        return nil;
    }

    // Force both reps to report the same logical size so NSImage picks the
    // right one based on screen density rather than treating them as two
    // unrelated images.
    let pt_size = NSSize { width: ICON_PT, height: ICON_PT };
    let _: () = msg_send![rep_1x, setSize: pt_size];
    let _: () = msg_send![rep_2x, setSize: pt_size];

    let cls_image = class!(NSImage);
    let image: id = msg_send![cls_image, alloc];
    let image: id = msg_send![image, init];
    if image == nil {
        return nil;
    }
    let _: () = msg_send![image, addRepresentation: rep_1x];
    let _: () = msg_send![image, addRepresentation: rep_2x];
    let _: () = msg_send![image, setSize: pt_size];

    image
}

/// Remove any electron subview we previously added to the button. Called
/// before installing a fresh one so a state change doesn't stack subviews.
unsafe fn remove_existing_electron(button: id) {
    let subviews: id = msg_send![button, subviews];
    let n: u64 = msg_send![subviews, count];
    for i in (0..n).rev() {
        let sub: id = msg_send![subviews, objectAtIndex: i];
        let tag: i64 = msg_send![sub, tag];
        if tag == ELECTRON_TAG {
            let _: () = msg_send![sub, removeFromSuperview];
        }
    }
}

/// Number of frames in the attention loop. Must match `ANIM_FRAMES` in
/// scripts/generate_tray_icons.py — both sides are checked into the repo so
/// regenerating the assets is a manual step.
pub const ANIM_FRAMES: usize = 24;

/// 1x PNGs for the measuring-state attention sweep, frame 0..ANIM_FRAMES-1.
/// Frame 0 sits at the resting angle, so the loop starts and ends in the
/// same pose as the static `tray-electron-measuring.png`.
const FRAMES_MEASURING_1X: [&[u8]; ANIM_FRAMES] = [
    include_bytes!("../icons/tray-electron-measuring-f00.png"),
    include_bytes!("../icons/tray-electron-measuring-f01.png"),
    include_bytes!("../icons/tray-electron-measuring-f02.png"),
    include_bytes!("../icons/tray-electron-measuring-f03.png"),
    include_bytes!("../icons/tray-electron-measuring-f04.png"),
    include_bytes!("../icons/tray-electron-measuring-f05.png"),
    include_bytes!("../icons/tray-electron-measuring-f06.png"),
    include_bytes!("../icons/tray-electron-measuring-f07.png"),
    include_bytes!("../icons/tray-electron-measuring-f08.png"),
    include_bytes!("../icons/tray-electron-measuring-f09.png"),
    include_bytes!("../icons/tray-electron-measuring-f10.png"),
    include_bytes!("../icons/tray-electron-measuring-f11.png"),
    include_bytes!("../icons/tray-electron-measuring-f12.png"),
    include_bytes!("../icons/tray-electron-measuring-f13.png"),
    include_bytes!("../icons/tray-electron-measuring-f14.png"),
    include_bytes!("../icons/tray-electron-measuring-f15.png"),
    include_bytes!("../icons/tray-electron-measuring-f16.png"),
    include_bytes!("../icons/tray-electron-measuring-f17.png"),
    include_bytes!("../icons/tray-electron-measuring-f18.png"),
    include_bytes!("../icons/tray-electron-measuring-f19.png"),
    include_bytes!("../icons/tray-electron-measuring-f20.png"),
    include_bytes!("../icons/tray-electron-measuring-f21.png"),
    include_bytes!("../icons/tray-electron-measuring-f22.png"),
    include_bytes!("../icons/tray-electron-measuring-f23.png"),
];

const FRAMES_MEASURING_2X: [&[u8]; ANIM_FRAMES] = [
    include_bytes!("../icons/tray-electron-measuring-f00@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f01@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f02@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f03@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f04@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f05@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f06@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f07@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f08@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f09@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f10@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f11@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f12@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f13@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f14@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f15@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f16@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f17@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f18@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f19@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f20@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f21@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f22@2x.png"),
    include_bytes!("../icons/tray-electron-measuring-f23@2x.png"),
];

fn electron_bytes(state: TrayState, frame: Option<usize>) -> (&'static [u8], &'static [u8]) {
    match state {
        TrayState::Measuring => match frame {
            Some(i) => {
                let i = i % ANIM_FRAMES;
                (FRAMES_MEASURING_1X[i], FRAMES_MEASURING_2X[i])
            }
            None => (
                include_bytes!("../icons/tray-electron-measuring.png"),
                include_bytes!("../icons/tray-electron-measuring@2x.png"),
            ),
        },
        TrayState::Reminder => (
            include_bytes!("../icons/tray-electron-reminder.png"),
            include_bytes!("../icons/tray-electron-reminder@2x.png"),
        ),
    }
}

/// Install (or replace) the coloured electron NSImageView on top of the
/// status-bar button. Safe to call repeatedly; old subviews are cleaned up.
/// Must be called on the main thread. Silently no-ops if the status item
/// can't be found yet (e.g. first call races the tray builder).
///
/// `frame = None` paints the resting electron for the given state.
/// `frame = Some(i)` paints the i-th animation frame of the measuring
/// sweep (used by `run_attention_animation`); the reminder state has no
/// per-frame variants and falls back to its resting electron.
pub fn apply_state(state: TrayState, frame: Option<usize>) {
    unsafe {
        let Some(item) = find_status_item() else { return };
        let button: id = msg_send![item, button];
        if button == nil {
            return;
        }

        remove_existing_electron(button);

        let (b1, b2) = electron_bytes(state, frame);
        let image = make_multires_nsimage(b1, b2);
        if image == nil {
            return;
        }
        let _: () = msg_send![image, setTemplate: NO];

        let bounds: NSRect = msg_send![button, bounds];
        let cls_view = class!(NSImageView);
        let view: id = msg_send![cls_view, alloc];
        let view: id = msg_send![view, initWithFrame: bounds];
        if view == nil {
            return;
        }
        let _: () = msg_send![view, setImage: image];
        let _: () = msg_send![view, setEditable: NO];
        // 3 == NSImageScaleProportionallyUpOrDown
        let _: () = msg_send![view, setImageScaling: 3_u64];
        let _: () = msg_send![view, setTag: ELECTRON_TAG];
        // 18 = NSViewWidthSizable | NSViewHeightSizable — keeps the
        // electron centred if the button bounds ever change.
        let _: () = msg_send![view, setAutoresizingMask: 18_u64];

        let _: () = msg_send![button, addSubview: view];
    }
}

/// Guards `run_attention_animation` against being kicked twice if the
/// first-launch path fires more than once (e.g. relocate marker + missing
/// onboarding flag). Cheaper and simpler than threading a stop-channel
/// since the loop is bounded by `ATTENTION_DURATION`.
static ATTENTION_RUNNING: AtomicBool = AtomicBool::new(false);

const FRAME_INTERVAL: Duration = Duration::from_millis(80);
const ATTENTION_DURATION: Duration = Duration::from_secs(6);

/// Spin the measuring-state electron around the ring for ~6 seconds to
/// draw the user's eye to the menu-bar icon during onboarding. Settles on
/// the resting frame when finished. Re-entrancy is a silent no-op.
pub fn run_attention_animation(app: tauri::AppHandle) {
    if ATTENTION_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        let total_frames =
            (ATTENTION_DURATION.as_millis() / FRAME_INTERVAL.as_millis()) as usize;
        for i in 0..total_frames {
            let frame = i % ANIM_FRAMES;
            let _ = app.run_on_main_thread(move || {
                apply_state(TrayState::Measuring, Some(frame));
            });
            std::thread::sleep(FRAME_INTERVAL);
        }
        let _ = app.run_on_main_thread(|| {
            apply_state(TrayState::Measuring, None);
        });
        ATTENTION_RUNNING.store(false, Ordering::SeqCst);
    });
}
