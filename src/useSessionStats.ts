import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface SessionStats {
  completed: number;
  total: number;
  today_count: number;
  current_streak: number;
}

const ZERO: SessionStats = { completed: 0, total: 0, today_count: 0, current_streak: 0 };

/**
 * Fetches the user's lifetime session counts from the local sessions.jsonl.
 * Used to derive the My Soul tier and filter out locked techniques.
 *
 * Re-fetches on `nonce` change — pass a value that changes when a session
 * completes so the panel reflects new totals immediately.
 */
export function useSessionStats(nonce: number = 0): SessionStats {
  const [stats, setStats] = useState<SessionStats>(ZERO);

  useEffect(() => {
    let cancelled = false;
    invoke<SessionStats>("get_session_stats")
      .then((s) => {
        if (!cancelled) setStats(s);
      })
      .catch(() => {
        /* leave previous stats; never block UI */
      });
    return () => {
      cancelled = true;
    };
  }, [nonce]);

  return stats;
}
