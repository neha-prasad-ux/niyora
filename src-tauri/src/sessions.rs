use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use crate::companion_sync::{CompanionSync, SessionWindow};
use crate::config;

/// Append-only JSONL log of breathing/mindfulness sessions, one per line.
/// Path: `~/Library/Application Support/com.niyora.breathing/sessions.jsonl`.
/// Forward-compatibility for v2 stats / streaks / "My Soul" surface.
/// Never read off-device — local-only by design.
fn sessions_path() -> Option<PathBuf> {
    Some(config::app_data_dir()?.join("sessions.jsonl"))
}

/// Append a session record to the given file. Pure function for testability ·
/// the Tauri command wraps this with the production path.
///
/// `session_id` is a stable UUID minted at record time. It is persisted in
/// the JSONL so the HRV companion can refer to a session by ID when it
/// reports back HRV deltas. Older rows without `session_id` predate the
/// companion feature and need no migration.
#[allow(clippy::too_many_arguments)]
fn append_session_line(
    path: &Path,
    session_id: &str,
    started_at: &str,
    technique_name: &str,
    technique_kind: &str,
    intended_duration_sec: u64,
    actual_duration_sec: u64,
    completed: bool,
) -> Result<(), String> {
    let entry = json!({
        "session_id": session_id,
        "started_at": started_at,
        "technique_name": technique_name,
        "technique_kind": technique_kind,
        "intended_duration_sec": intended_duration_sec,
        "actual_duration_sec": actual_duration_sec,
        "completed": completed,
    });
    let line = entry.to_string();
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok(())
}

/// Compute the RFC 3339 end timestamp given an RFC 3339 start and a duration
/// in seconds. Returns `None` if the start string cannot be parsed; in that
/// case callers skip the companion emit rather than send a bogus window.
fn compute_end(started_at: &str, actual_duration_sec: u64) -> Option<String> {
    let start: DateTime<Utc> = DateTime::parse_from_rfc3339(started_at)
        .ok()?
        .with_timezone(&Utc);
    let end = start.checked_add_signed(Duration::seconds(actual_duration_sec as i64))?;
    Some(end.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Record a completed or abandoned session.
///
/// Called from the frontend on:
///   - successful completion (after the "well done" screen finishes)
///   - early close past the intro (abandoned)
///
/// Closes during the 3s intro are not recorded — the user didn't really start.
#[tauri::command]
pub fn record_session(
    started_at: String,
    technique_name: String,
    technique_kind: String,
    intended_duration_sec: u64,
    actual_duration_sec: u64,
    completed: bool,
    companion: tauri::State<'_, CompanionSync>,
) -> Result<(), String> {
    let path = sessions_path().ok_or_else(|| "Could not resolve sessions path".to_string())?;
    let session_id = uuid::Uuid::new_v4().to_string();
    append_session_line(
        &path,
        &session_id,
        &started_at,
        &technique_name,
        &technique_kind,
        intended_duration_sec,
        actual_duration_sec,
        completed,
    )?;

    // Anonymous opt-in telemetry. No-op unless the user opted in on the
    // onboarding consent slide. Only the keys listed in `telemetry::forwarded_meta`
    // are forwarded to PostHog.
    crate::telemetry::capture(
        "session_recorded",
        &serde_json::json!({
            "technique_name": technique_name,
            "technique_kind": technique_kind,
            "completed": completed,
        }),
    );

    // Emit to the HRV companion only for completed sessions. Abandoned
    // sessions are not interesting for HRV impact measurement because the
    // user did not finish the practice. If `started_at` is malformed we
    // skip the emit silently rather than send a window with no `end`.
    if completed {
        if let Some(end) = compute_end(&started_at, actual_duration_sec) {
            companion.enqueue_window(SessionWindow {
                session_id,
                start: started_at,
                end,
                technique_name,
            });
        }
    }

    Ok(())
}

/// Aggregate session stats for the My Soul panel.
/// Only completed sessions count toward `completed` — abandoned sessions
/// are recorded but don't progress the user's tier.
#[derive(Serialize)]
pub struct SessionStats {
    pub completed: u32,
    pub total: u32,
}

fn count_lines(path: &Path) -> SessionStats {
    let mut completed = 0u32;
    let mut total = 0u32;
    let Ok(file) = std::fs::File::open(path) else {
        return SessionStats { completed, total };
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        total = total.saturating_add(1);
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if v.get("completed").and_then(|c| c.as_bool()).unwrap_or(false) {
                completed = completed.saturating_add(1);
            }
        }
    }
    SessionStats { completed, total }
}

/// Dev-only: deletes the sessions log so the user appears as a brand new
/// install (zero sessions). Wired to a "Reset sessions" tray menu item that
/// only appears in debug builds. Used to verify first-session behaviour like
/// the Box Breath default and the AHA screen.
#[cfg(debug_assertions)]
pub fn reset_sessions() -> Result<(), String> {
    let Some(path) = sessions_path() else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_session_stats() -> SessionStats {
    let Some(path) = sessions_path() else {
        return SessionStats {
            completed: 0,
            total: 0,
        };
    };
    count_lines(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    fn temp_path(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("niyora-test-{label}-{nanos}.jsonl"))
    }

    #[test]
    fn appends_one_completed_session() {
        let path = temp_path("completed");
        let _ = fs::remove_file(&path);

        append_session_line(
            &path,
            "11111111-1111-1111-1111-111111111111",
            "2026-04-26T10:30:00Z",
            "Box Breathing",
            "breathing",
            64,
            64,
            true,
        )
        .expect("write should succeed");

        let contents = fs::read_to_string(&path).expect("file should exist");
        let parsed: Value = serde_json::from_str(contents.trim()).expect("valid JSON");

        assert_eq!(parsed["session_id"], "11111111-1111-1111-1111-111111111111");
        assert_eq!(parsed["started_at"], "2026-04-26T10:30:00Z");
        assert_eq!(parsed["technique_name"], "Box Breathing");
        assert_eq!(parsed["technique_kind"], "breathing");
        assert_eq!(parsed["intended_duration_sec"], 64);
        assert_eq!(parsed["actual_duration_sec"], 64);
        assert_eq!(parsed["completed"], true);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn appends_multiple_sessions_one_per_line() {
        let path = temp_path("multi");
        let _ = fs::remove_file(&path);

        append_session_line(&path, "id-1", "ts1", "Ujjayi", "breathing", 60, 60, true).unwrap();
        append_session_line(&path, "id-2", "ts2", "Trataka", "mindfulness", 33, 12, false).unwrap();
        append_session_line(&path, "id-3", "ts3", "4-7-8 Breath", "breathing", 76, 76, true).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);

        let line2: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(line2["session_id"], "id-2");
        assert_eq!(line2["technique_name"], "Trataka");
        assert_eq!(line2["completed"], false);
        assert_eq!(line2["actual_duration_sec"], 12);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn count_lines_counts_only_completed() {
        let path = temp_path("counts");
        let _ = fs::remove_file(&path);

        append_session_line(&path, "id-a", "ts1", "Box Breath", "breathing", 65, 65, true).unwrap();
        append_session_line(&path, "id-b", "ts2", "Ujjayi", "breathing", 60, 12, false).unwrap();
        append_session_line(&path, "id-c", "ts3", "Belly Breath", "breathing", 60, 60, true).unwrap();
        append_session_line(&path, "id-d", "ts4", "Trataka", "mindfulness", 33, 5, false).unwrap();

        let stats = count_lines(&path);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.completed, 2);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn count_lines_handles_missing_file() {
        let path = temp_path("missing");
        let _ = fs::remove_file(&path);
        let stats = count_lines(&path);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.completed, 0);
    }

    #[test]
    fn records_abandoned_session_with_partial_duration() {
        let path = temp_path("abandoned");
        let _ = fs::remove_file(&path);

        append_session_line(
            &path,
            "22222222-2222-2222-2222-222222222222",
            "2026-04-26T11:00:00Z",
            "Self Compassion",
            "mindfulness",
            24,
            8,
            false,
        )
        .unwrap();

        let parsed: Value =
            serde_json::from_str(fs::read_to_string(&path).unwrap().trim()).unwrap();
        assert_eq!(parsed["completed"], false);
        assert_eq!(parsed["actual_duration_sec"], 8);
        assert!(
            parsed["actual_duration_sec"].as_u64().unwrap()
                < parsed["intended_duration_sec"].as_u64().unwrap(),
            "abandoned session should have actual < intended"
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn compute_end_adds_actual_duration_to_start() {
        // 64s session ending one minute and four seconds after start.
        let end = compute_end("2026-05-20T10:00:00Z", 64).expect("valid RFC 3339 in");
        assert_eq!(end, "2026-05-20T10:01:04Z");
    }

    #[test]
    fn compute_end_returns_none_on_bad_start() {
        assert!(compute_end("not a date", 60).is_none());
        assert!(compute_end("", 60).is_none());
    }
}
