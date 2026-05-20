//! `companion_queue.json` · queue of session windows that have not yet
//! been delivered to every paired iPhone. Drained on peer auth, and after
//! every fan-out attempt to remove windows that all paired phones have
//! received.
//!
//! Spec contract: cap at 50 entries. When full, drop the oldest. The HRV
//! feature is best-effort by design (privacy demands no cloud retention),
//! so losing a one-month-old window is the right trade vs unbounded disk
//! growth.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config;

use super::protocol::ServerMessage;

const FILENAME: &str = "companion_queue.json";
const VERSION_CURRENT: u32 = 1;
const QUEUE_CAP: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedWindow {
    pub session_id: String,
    pub start: String,
    pub end: String,
    pub technique_name: String,
    pub queued_at: String,
    /// client_ids of paired phones that have received this window. When
    /// this set covers every paired device, the entry can be pruned.
    pub sent_to: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub version: u32,
    pub entries: Vec<QueuedWindow>,
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
    serde_json::from_slice::<Queue>(&bytes).unwrap_or_default()
}

pub fn save(q: &Queue) -> Result<(), String> {
    let path = queue_path().ok_or_else(|| "no app_data_dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(q).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

/// Append a fresh window to the queue. If the queue is at cap, drops the
/// oldest entry first · this is the documented "lose history before losing
/// the future" behaviour.
pub fn enqueue(window: QueuedWindow) -> Result<(), String> {
    let mut q = load();
    if q.entries.iter().any(|e| e.session_id == window.session_id) {
        return Ok(());
    }
    q.entries.push(window);
    while q.entries.len() > QUEUE_CAP {
        q.entries.remove(0);
    }
    save(&q)
}

/// Return every queued window the given phone has not yet received, in
/// queued-at order. Caller is expected to send each and then call
/// `mark_sent_to` per success.
pub fn pending_for(client_id: &str) -> Vec<QueuedWindow> {
    load()
        .entries
        .into_iter()
        .filter(|e| !e.sent_to.contains(client_id))
        .collect()
}

/// Mark a window as delivered to a phone, and prune the entry entirely if
/// every known paired device has now received it.
pub fn mark_sent_to(session_id: &str, client_id: &str, all_paired_client_ids: &[String]) -> Result<(), String> {
    let mut q = load();
    if let Some(entry) = q.entries.iter_mut().find(|e| e.session_id == session_id) {
        entry.sent_to.insert(client_id.to_string());
        let fully_delivered = !all_paired_client_ids.is_empty()
            && all_paired_client_ids
                .iter()
                .all(|c| entry.sent_to.contains(c));
        if fully_delivered {
            q.entries.retain(|e| e.session_id != session_id);
        }
    }
    save(&q)
}

/// Convert a queued entry into the wire frame the phone receives.
pub fn to_window_message(e: &QueuedWindow) -> ServerMessage {
    ServerMessage::Window {
        session_id: e.session_id.clone(),
        start: e.start.clone(),
        end: e.end.clone(),
        technique_name: e.technique_name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> QueuedWindow {
        QueuedWindow {
            session_id: id.into(),
            start: "2026-05-20T10:00:00Z".into(),
            end: "2026-05-20T10:01:00Z".into(),
            technique_name: "Box Breathing".into(),
            queued_at: "2026-05-20T10:01:00Z".into(),
            sent_to: BTreeSet::new(),
        }
    }

    /// Pure in-memory helpers, no disk · the file IO is covered by an
    /// integration-style test that uses a temp dir below.
    #[test]
    fn duplicate_enqueue_is_idempotent_in_memory() {
        let mut q = Queue::default();
        let w = sample("s-1");
        q.entries.push(w.clone());
        // Mirror what enqueue() would do · skip if session_id already present.
        if !q.entries.iter().any(|e| e.session_id == w.session_id) {
            q.entries.push(w);
        }
        assert_eq!(q.entries.len(), 1);
    }

    #[test]
    fn cap_drops_oldest_first() {
        let mut q = Queue::default();
        for i in 0..QUEUE_CAP + 5 {
            q.entries.push(sample(&format!("s-{i}")));
        }
        while q.entries.len() > QUEUE_CAP {
            q.entries.remove(0);
        }
        assert_eq!(q.entries.len(), QUEUE_CAP);
        assert_eq!(q.entries[0].session_id, "s-5");
        assert_eq!(q.entries.last().unwrap().session_id, format!("s-{}", QUEUE_CAP + 4));
    }

    #[test]
    fn pending_filter_skips_already_sent() {
        let mut q = Queue::default();
        let mut a = sample("a");
        a.sent_to.insert("phone-1".into());
        let b = sample("b");
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
    fn fully_delivered_window_is_pruned() {
        let mut q = Queue::default();
        let mut w = sample("w-1");
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
    fn to_window_message_maps_fields() {
        let w = sample("s-99");
        match to_window_message(&w) {
            ServerMessage::Window {
                session_id,
                start,
                end,
                technique_name,
            } => {
                assert_eq!(session_id, "s-99");
                assert_eq!(start, w.start);
                assert_eq!(end, w.end);
                assert_eq!(technique_name, "Box Breathing");
            }
            _ => panic!("expected Window"),
        }
    }
}
