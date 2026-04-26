use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::config;

/// Append-only JSONL log of breathing/mindfulness sessions, one per line.
/// Path: `~/Library/Application Support/com.niyora.breathing/sessions.jsonl`.
/// Forward-compatibility for v2 stats / streaks / "My Soul" surface.
/// Never read off-device — local-only by design.
fn sessions_path() -> Option<PathBuf> {
    Some(config::app_data_dir()?.join("sessions.jsonl"))
}

/// Append a session record to the given file. Pure function for testability —
/// the Tauri command wraps this with the production path.
fn append_session_line(
    path: &Path,
    started_at: &str,
    technique_name: &str,
    technique_kind: &str,
    intended_duration_sec: u64,
    actual_duration_sec: u64,
    completed: bool,
) -> Result<(), String> {
    let entry = json!({
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
) -> Result<(), String> {
    let path = sessions_path().ok_or_else(|| "Could not resolve sessions path".to_string())?;
    append_session_line(
        &path,
        &started_at,
        &technique_name,
        &technique_kind,
        intended_duration_sec,
        actual_duration_sec,
        completed,
    )
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

        append_session_line(&path, "ts1", "Ujjayi", "breathing", 60, 60, true).unwrap();
        append_session_line(&path, "ts2", "Trataka", "mindfulness", 33, 12, false).unwrap();
        append_session_line(&path, "ts3", "4-7-8 Breath", "breathing", 76, 76, true).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);

        let line2: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(line2["technique_name"], "Trataka");
        assert_eq!(line2["completed"], false);
        assert_eq!(line2["actual_duration_sec"], 12);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn count_lines_counts_only_completed() {
        let path = temp_path("counts");
        let _ = fs::remove_file(&path);

        append_session_line(&path, "ts1", "Box Breath", "breathing", 65, 65, true).unwrap();
        append_session_line(&path, "ts2", "Ujjayi", "breathing", 60, 12, false).unwrap();
        append_session_line(&path, "ts3", "Belly Breath", "breathing", 60, 60, true).unwrap();
        append_session_line(&path, "ts4", "Trataka", "mindfulness", 33, 5, false).unwrap();

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
}
