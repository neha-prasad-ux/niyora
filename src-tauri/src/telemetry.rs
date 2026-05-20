// Anonymous, opt-in analytics via PostHog.
//
// Privacy contract:
// - Nothing is ever sent unless the user explicitly opts in on the
//   onboarding consent slide (`analytics_consent == Some(true)`).
// - Only the event types in REMOTE_ALLOWLIST leave the device. Anything
//   pss4-related (mental-health data) is deliberately excluded.
// - Per-event meta is also allow-listed in `forwarded_meta`. Notably
//   `reminder_fired` keeps `interval_min` and `snoozed` but drops `score`
//   and `label`. The stress reading itself never leaves the device.
// - The anonymous id is a random UUID generated once on opt-in. It is not
//   derived from anything and is not tied to any personal information.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::config;

// PostHog project API key. This is the public / write-only key and is safe
// to ship inside the client.
const PH_API_KEY: &str = "phc_rsYNhJPygj92QaVhz2KCCm4WozFkPLRUFupXkVyZSNMG";

// PostHog ingestion host. The Niyora project is on PostHog EU cloud.
const PH_HOST: &str = "https://eu.i.posthog.com";

// The only event types that may ever be sent to PostHog. Everything else
// stays local in events.jsonl. pss4 / pss4_dismissed are intentionally
// absent: stress check-in data never leaves the device.
const REMOTE_ALLOWLIST: &[&str] = &[
    "app_launched",
    "session_recorded",
    "reminder_fired",
    "snooze_started",
];

/// Per-event allow-list of meta keys that may be forwarded. Any key not
/// listed here is dropped before the event leaves the device.
fn forwarded_meta(event_type: &str, meta: &Value) -> serde_json::Map<String, Value> {
    let keys: &[&str] = match event_type {
        "session_recorded" => &["technique_name", "technique_kind", "completed"],
        "reminder_fired" => &["interval_min", "snoozed"],
        "snooze_started" => &["duration_minutes", "source"],
        _ => &[],
    };
    let mut out = serde_json::Map::new();
    for key in keys {
        if let Some(v) = meta.get(*key) {
            out.insert((*key).to_string(), v.clone());
        }
    }
    out
}

/// Current analytics consent: `None` until the user answers on the
/// onboarding consent slide, then `Some(true)` / `Some(false)`.
#[tauri::command]
pub fn analytics_consent_status() -> Option<bool> {
    config::load().analytics_consent
}

/// Records the user's choice from the onboarding consent slide. On opt-in,
/// generates a one-time anonymous id if there isn't one already.
#[tauri::command]
pub fn set_analytics_consent(granted: bool) -> Result<(), String> {
    let mut cfg = config::load();
    cfg.analytics_consent = Some(granted);
    if granted && cfg.analytics_id.is_none() {
        cfg.analytics_id = Some(Uuid::new_v4().to_string());
    }
    config::save(&cfg)
}

/// Fire-and-forget: send one anonymous event to PostHog, but only if the
/// event type is allow-listed AND the user has opted in. No-op otherwise.
/// Only the meta keys in `forwarded_meta` are included. Never blocks the
/// caller and never surfaces errors. Telemetry must not affect app behavior.
pub fn capture(event_type: &str, meta: &Value) {
    if !REMOTE_ALLOWLIST.contains(&event_type) {
        return;
    }
    let cfg = config::load();
    if cfg.analytics_consent != Some(true) {
        return;
    }
    let Some(distinct_id) = cfg.analytics_id else {
        return;
    };

    let extra = forwarded_meta(event_type, meta);
    let event_type = event_type.to_string();
    std::thread::spawn(move || {
        let mut properties = serde_json::Map::new();
        properties.insert("$lib".into(), json!("niyora"));
        properties.insert("app_version".into(), json!(env!("CARGO_PKG_VERSION")));
        properties.insert("os".into(), json!(std::env::consts::OS));
        for (k, v) in extra {
            properties.insert(k, v);
        }
        let payload = json!({
            "api_key": PH_API_KEY,
            "event": event_type,
            "distinct_id": distinct_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "properties": properties,
        });
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let _ = client
            .post(format!("{PH_HOST}/i/v0/e/"))
            .json(&payload)
            .send();
    });
}
