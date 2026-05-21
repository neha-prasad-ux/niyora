//! `companion_queue.json` · queue of measurement requests that have not
//! yet been delivered to every paired iPhone. Drained on peer auth, and
//! after every fan-out attempt to remove requests every paired phone has
//! received.
//!
//! Spec contract: cap at 50 entries. When full, drop the oldest. The HRV
//! feature is best-effort by design (privacy demands no cloud retention),
//! so losing a one-month-old request is the right trade vs unbounded disk
//! growth.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;

use super::protocol::ServerMessage;

const FILENAME: &str = "companion_queue.json";
const VERSION_CURRENT: u32 = 2;
const QUEUE_CAP: usize = 50;

/// One pending `request_measurement` frame. Keyed by (session_id, phase)
/// · the user can tap "Measure stress" for both phases of the same
/// session, and each tap produces a separate queue entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedRequest {
    pub session_id: String,
    pub phase: String,
    pub technique_name: String,
    pub queued_at: String,
    /// client_ids of paired phones that have received this request. When
    /// this set covers every paired device, the entry can be pruned.
    pub sent_to: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub version: u32,
    pub entries: Vec<QueuedRequest>,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            version: VERSION_CURRENT,
            entries: Vec::new(),
        }
    }
}

fn queue_path() -> Option<PathBuf> {
    Some(config::app_data_dir()?.join(FILENAME))
}

pub fn load() -> Queue {
    let Some(path) = queue_path() else {
        return Queue::default();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return Queue::default();
    };
    parse(&bytes)
}

/// Pure parse for testability. v1 queue files (session windows from the
/// removed HealthKit auto-read path) are dropped on load · the request
/// shape changed and resurrecting old windows would produce useless
/// frames for the new iOS code.
fn parse(bytes: &[u8]) -> Queue {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Queue::default();
    };
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version != VERSION_CURRENT {
        return Queue::default();
    }
    serde_json::from_value::<Queue>(value).unwrap_or_default()
}

pub fn save(q: &Queue) -> Result<(), String> {
    let path = queue_path().ok_or_else(|| "no app_data_dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(q).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Append a fresh request to the queue. If the queue is at cap, drops the
/// oldest entry first · this is the documented "lose history before losing
/// the future" behaviour. Idempotent on (session_id, phase).
pub fn enqueue(req: QueuedRequest) -> Result<(), String> {
    let mut q = load();
    if q.entries
        .iter()
        .any(|e| e.session_id == req.session_id && e.phase == req.phase)
    {
        return Ok(());
    }
    q.entries.push(req);
    while q.entries.len() > QUEUE_CAP {
        q.entries.remove(0);
    }
    save(&q)
}

/// Return every queued request the given phone has not yet received, in
/// queued-at order. Caller is expected to send each and then call
/// `mark_sent_to` per success.
pub fn pending_for(client_id: &str) -> Vec<QueuedRequest> {
    load()
        .entries
        .into_iter()
        .filter(|e| !e.sent_to.contains(client_id))
        .collect()
}

/// Mark a request as delivered to a phone, and prune the entry entirely
/// if every known paired device has now received it.
pub fn mark_sent_to(
    session_id: &str,
    phase: &str,
    client_id: &str,
    all_paired_client_ids: &[String],
) -> Result<(), String> {
    let mut q = load();
    if let Some(entry) = q
        .entries
        .iter_mut()
        .find(|e| e.session_id == session_id && e.phase == phase)
    {
        entry.sent_to.insert(client_id.to_string());
        let fully_delivered = !all_paired_client_ids.is_empty()
            && all_paired_client_ids
                .iter()
                .all(|c| entry.sent_to.contains(c));
        if fully_delivered {
            q.entries
                .retain(|e| !(e.session_id == session_id && e.phase == phase));
        }
    }
    save(&q)
}

/// Convert a queued entry into the wire frame the phone receives.
pub fn to_request_message(e: &QueuedRequest) -> ServerMessage {
    ServerMessage::RequestMeasurement {
        session_id: e.session_id.clone(),
        phase: e.phase.clone(),
        technique_name: e.technique_name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, phase: &str) -> QueuedRequest {
        QueuedRequest {
            session_id: id.into(),
            phase: phase.into(),
            technique_name: "Box Breathing".into(),
            queued_at: "2026-05-20T10:01:00Z".into(),
            sent_to: BTreeSet::new(),
        }
    }

    #[test]
    fn duplicate_enqueue_is_idempotent_in_memory() {
        let mut q = Queue::default();
        let w = sample("s-1", "pre");
        q.entries.push(w.clone());
        if !q
            .entries
            .iter()
            .any(|e| e.session_id == w.session_id && e.phase == w.phase)
        {
            q.entries.push(w);
        }
        assert_eq!(q.entries.len(), 1);
    }

    #[test]
    fn pre_and_post_for_same_session_are_distinct_entries() {
        let mut q = Queue::default();
        q.entries.push(sample("s-1", "pre"));
        // Adding the post entry must not be deduped by session_id alone.
        let post = sample("s-1", "post");
        if !q
            .entries
            .iter()
            .any(|e| e.session_id == post.session_id && e.phase == post.phase)
        {
            q.entries.push(post);
        }
        assert_eq!(q.entries.len(), 2);
    }

    #[test]
    fn cap_drops_oldest_first() {
        let mut q = Queue::default();
        for i in 0..QUEUE_CAP + 5 {
            q.entries.push(sample(&format!("s-{i}"), "post"));
        }
        while q.entries.len() > QUEUE_CAP {
            q.entries.remove(0);
        }
        assert_eq!(q.entries.len(), QUEUE_CAP);
        assert_eq!(q.entries[0].session_id, "s-5");
        assert_eq!(
            q.entries.last().unwrap().session_id,
            format!("s-{}", QUEUE_CAP + 4)
        );
    }

    #[test]
    fn pending_filter_skips_already_sent() {
        let mut q = Queue::default();
        let mut a = sample("a", "post");
        a.sent_to.insert("phone-1".into());
        let b = sample("b", "post");
        q.entries.push(a);
        q.entries.push(b);

        let pending: Vec<_> = q
            .entries
            .iter()
            .filter(|e| !e.sent_to.contains("phone-1"))
            .map(|e| e.session_id.clone())
            .collect();
        assert_eq!(pending, vec!["b".to_string()]);
    }

    #[test]
    fn fully_delivered_request_is_pruned() {
        let mut q = Queue::default();
        let mut w = sample("w-1", "post");
        w.sent_to.insert("phone-1".into());
        w.sent_to.insert("phone-2".into());
        q.entries.push(w);

        let paired = vec!["phone-1".to_string(), "phone-2".to_string()];
        let fully = q.entries[0]
            .sent_to
            .iter()
            .all(|c| paired.contains(c))
            && paired.len() == q.entries[0].sent_to.len();
        if fully {
            q.entries.clear();
        }
        assert!(q.entries.is_empty());
    }

    #[test]
    fn to_request_message_maps_fields() {
        let w = sample("s-99", "pre");
        match to_request_message(&w) {
            ServerMessage::RequestMeasurement {
                session_id,
                phase,
                technique_name,
            } => {
                assert_eq!(session_id, "s-99");
                assert_eq!(phase, "pre");
                assert_eq!(technique_name, "Box Breathing");
            }
            _ => panic!("expected RequestMeasurement"),
        }
    }

    #[test]
    fn v1_queue_file_is_dropped() {
        let v1 = br#"{
            "version": 1,
            "entries": [{
                "session_id": "old-1",
                "start": "2026-04-01T10:00:00Z",
                "end": "2026-04-01T10:01:00Z",
                "technique_name": "Box",
                "queued_at": "2026-04-01T10:01:00Z",
                "sent_to": []
            }]
        }"#;
        let q = parse(v1);
        assert!(q.entries.is_empty());
        assert_eq!(q.version, VERSION_CURRENT);
    }
}
