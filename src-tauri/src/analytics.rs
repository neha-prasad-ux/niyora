use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use chrono::{Datelike, Weekday};
use serde::Serialize;
use serde_json::{json, Value};

use crate::config;

fn events_path() -> Option<PathBuf> {
    Some(config::app_data_dir()?.join("events.jsonl"))
}

/// Append-only JSONL event log. One event per line.
pub fn append_event(event_type: &str, meta: Value) -> Result<(), String> {
    let path = events_path().ok_or_else(|| "Could not resolve events path".to_string())?;
    let entry = json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "type": event_type,
        "meta": meta,
    });
    let line = entry.to_string();
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    crate::telemetry::capture(event_type, &meta);
    Ok(())
}

#[tauri::command]
pub fn log_event(event_type: String, meta: Option<Value>) -> Result<(), String> {
    append_event(&event_type, meta.unwrap_or_else(|| json!({})))
}

// --- PSS-4 weekly check-in (Cohen 1983, 4-item validated) ---

fn today_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn mark_pss4_done() -> Result<(), String> {
    let mut cfg = config::load();
    cfg.last_pss4_date = Some(today_iso());
    config::save(&cfg)
}

fn is_today_sunday(weekday: Weekday) -> bool {
    weekday == Weekday::Sun
}

/// Returns true if the panel should auto-prompt the PSS-4 right now:
/// today is Sunday AND the user hasn't completed/dismissed it today.
#[tauri::command]
pub fn should_show_pss4() -> bool {
    let today = chrono::Local::now();
    if !is_today_sunday(today.weekday()) {
        return false;
    }
    let cfg = config::load();
    cfg.last_pss4_date != Some(today_iso())
}

/// Pure scoring logic for PSS-4. Items 2 and 3 (0-indexed: 1 and 2) are
/// positively-worded and reverse-scored per the validated instrument.
/// Returns total 0..=16 (higher = more perceived stress).
fn score_pss4(answers: &[u8]) -> Result<u8, String> {
    if answers.len() != 4 {
        return Err("PSS-4 requires exactly 4 answers".to_string());
    }
    let clip = |x: u8| x.min(4);
    let q1 = clip(answers[0]);
    let q2 = 4 - clip(answers[1]); // reverse
    let q3 = 4 - clip(answers[2]); // reverse
    let q4 = clip(answers[3]);
    Ok(q1 + q2 + q3 + q4)
}

/// Score a PSS-4 submission and log it. `answers` is a 4-element array of
/// 0..=4 responses on the Never→Very Often scale.
#[tauri::command]
pub fn submit_pss4(answers: Vec<u8>) -> Result<u8, String> {
    let total = score_pss4(&answers)?;
    append_event(
        "pss4",
        json!({
            "answers": answers,
            "score": total,
        }),
    )?;
    mark_pss4_done()?;
    Ok(total)
}

/// Mark this Sunday as handled without recording a score (user skipped).
#[tauri::command]
pub fn dismiss_pss4() -> Result<(), String> {
    append_event("pss4_dismissed", json!({}))?;
    mark_pss4_done()
}

/// One PSS-4 score for the history sparkline.
#[derive(Serialize)]
pub struct Pss4Entry {
    /// Local YYYY-MM-DD date (extracted from the event timestamp).
    pub date: String,
    /// Total PSS-4 score, 0..=16. Higher = more perceived stress.
    pub score: u8,
}

/// Read all PSS-4 scores recorded in events.jsonl, ordered oldest → newest.
/// Used by the My Soul panel to draw a comparison sparkline so the score has
/// value beyond the moment-of-completion.
#[tauri::command]
pub fn pss4_history() -> Vec<Pss4Entry> {
    let Some(path) = events_path() else {
        return Vec::new();
    };
    let Ok(file) = std::fs::File::open(&path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v.get("type").and_then(|t| t.as_str()) != Some("pss4") {
            continue;
        }
        let score = v
            .get("meta")
            .and_then(|m| m.get("score"))
            .and_then(|s| s.as_u64())
            .map(|s| s as u8);
        let ts = v.get("ts").and_then(|t| t.as_str());
        if let (Some(score), Some(ts)) = (score, ts) {
            // ts is RFC3339 UTC; convert to local YYYY-MM-DD for display.
            let date = chrono::DateTime::parse_from_rfc3339(ts)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
                .unwrap_or_else(|_| ts.chars().take(10).collect());
            out.push(Pss4Entry { date, score });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    // --- score_pss4 ---

    #[test]
    fn score_pss4_minimum() {
        // Forward items at 0, reverse items at 4: each contributes 0.
        assert_eq!(score_pss4(&[0, 4, 4, 0]), Ok(0));
    }

    #[test]
    fn score_pss4_maximum() {
        // Forward items at 4, reverse items at 0: each contributes 4.
        assert_eq!(score_pss4(&[4, 0, 0, 4]), Ok(16));
    }

    #[test]
    fn score_pss4_mixed() {
        // 1 + (4-2) + (4-3) + 0 = 1 + 2 + 1 + 0 = 4
        assert_eq!(score_pss4(&[1, 2, 3, 0]), Ok(4));
    }

    #[test]
    fn score_pss4_wrong_length() {
        assert!(score_pss4(&[]).is_err());
        assert!(score_pss4(&[1, 2, 3]).is_err());
        assert!(score_pss4(&[1, 2, 3, 4, 5]).is_err());
    }

    #[test]
    fn score_pss4_clips_out_of_range_answers() {
        // Answers > 4 are silently clipped to 4. Without clipping,
        // `4 - 5_u8` would panic in debug mode (u8 underflow) on the
        // reverse-scored items. The clip is the safety contract.
        // Forward items at 5 behave identically to 4.
        assert_eq!(score_pss4(&[5, 0, 0, 5]), Ok(16)); // clip(5)+4+4+clip(5)
        // Reverse items at 5: 4-clip(5)=0 → max perceived-high-stress.
        assert_eq!(score_pss4(&[0, 5, 5, 0]), Ok(0));  // 0+(4-4)+(4-4)+0
        // u8::MAX clips without wrapping.
        assert_eq!(score_pss4(&[255, 0, 0, 255]), Ok(16));
    }

    #[test]
    fn score_pss4_all_zeros_produces_8_not_0() {
        // Answering "Never" (0) to every question is NOT minimum stress.
        // Items 2 and 3 are reverse-scored, so "Never felt confident" and
        // "Never felt things were going your way" each contribute 4 points.
        // 0 + (4-0) + (4-0) + 0 = 8.
        assert_eq!(score_pss4(&[0, 0, 0, 0]), Ok(8));
    }

    // --- is_today_sunday ---

    #[test]
    fn is_today_sunday_false_for_non_sunday() {
        assert!(!is_today_sunday(Weekday::Mon));
        assert!(!is_today_sunday(Weekday::Fri));
        assert!(!is_today_sunday(Weekday::Sat));
    }

    #[test]
    fn is_today_sunday_true_for_sunday() {
        assert!(is_today_sunday(Weekday::Sun));
    }

    // --- pss4_history ---

    #[test]
    fn pss4_history_entries_are_well_formed() {
        // pss4_history must never panic. In CI (no events.jsonl yet) it
        // returns an empty Vec and exercises the File::open Err early-return.
        // On a dev machine with history it validates every entry's invariants.
        let entries = pss4_history();
        for e in &entries {
            assert!(
                e.score <= 16,
                "PSS-4 score {} out of valid range 0..=16",
                e.score
            );
            assert!(!e.date.is_empty(), "date field must not be empty");
        }
    }
}
