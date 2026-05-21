import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { calmScore } from "./calmScore";

/** One row of the companion HRV history JSON. Mirrors the Rust shape. */
interface PhaseCapture {
  rmssd_ms: number | null;
  sdnn_ms: number | null;
  sample_count: number;
  snr_db: number | null;
  status: string;
  received_at: string;
}
interface HrvReading {
  session_id: string;
  first_seen_at: string;
  pre: PhaseCapture | null;
  post: PhaseCapture | null;
}

interface Props {
  /** Current session id (matches the row the iPhone sent results for). */
  sessionId: string;
  /** Close the panel. */
  onDone: () => void;
  /** Open the explainer sheet. Stubbed for v1 · just dismisses. */
  onLearnMore: () => void;
  /** Open My Soul with the trend chart focused. */
  onSeeSoul: () => void;
}

/**
 * The "After breath" reveal screen shown when both pre and post PPG
 * captures landed for the current session. Two same-family orbs at the
 * top (the BEFORE pulses fast, the AFTER pulses slow), one delta line
 * "X% calmer," a tappable 5-session micro-trend, and Learn more / Done.
 *
 * Hidden entirely if there is not yet both a pre and post for this
 * session · the caller falls back to the regular mood-dots flow in that
 * case.
 */
export default function PostSessionReveal({
  sessionId,
  onDone,
  onLearnMore,
  onSeeSoul,
}: Props) {
  const [current, setCurrent] = useState<HrvReading | null>(null);
  const [recent, setRecent] = useState<HrvReading[]>([]);

  useEffect(() => {
    let cancelled = false;
    invoke<HrvReading[]>("companion_hrv_history")
      .then((rs) => {
        if (cancelled) return;
        const here = rs.find((r) => r.session_id === sessionId) ?? null;
        setCurrent(here);
        // Most recent 5 ok-status sessions for the trend strip, oldest
        // first so today's pair sits on the right.
        const ok = rs
          .filter((r) => r.post?.status === "ok" && r.pre?.status === "ok")
          .slice()
          .sort((a, b) => Date.parse(a.first_seen_at) - Date.parse(b.first_seen_at));
        setRecent(ok.slice(-5));
      })
      .catch(() => {
        if (cancelled) return;
        setCurrent(null);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  if (!current || !current.pre || !current.post) return null;
  if (current.pre.status !== "ok" || current.post.status !== "ok") return null;
  const preMs = current.pre.rmssd_ms;
  const postMs = current.post.rmssd_ms;
  if (typeof preMs !== "number" || typeof postMs !== "number") return null;

  const preCalm = calmScore(preMs);
  const postCalm = calmScore(postMs);
  if (preCalm === null || postCalm === null) return null;

  // Calm score is on a 0-100 scale, so a point delta IS the percentage
  // change in calm score. "20% calmer" reads naturally when the score
  // moved 53 → 73. The min(0) guard handles a session where the user
  // actually came in calmer than they left (rare but possible).
  const deltaPct = Math.round(postCalm - preCalm);
  const direction = deltaPct >= 0 ? "calmer" : "more activated";
  const absPct = Math.abs(deltaPct);

  return (
    <div className="niyora-reveal">
      <div className="reveal-backdrop" />
      <div className="reveal-content">
        <div className="reveal-eyebrow">After breath</div>

        <div className="reveal-pair">
          <div className="reveal-col">
            <div className="reveal-orb reveal-orb-before" />
            <div className="reveal-state">Before</div>
          </div>
          <div className="reveal-col">
            <div className="reveal-orb reveal-orb-after" />
            <div className="reveal-state">After</div>
          </div>
        </div>

        <div className="reveal-delta">
          {absPct}% {direction}
        </div>

        <button
          type="button"
          className="reveal-trend"
          onClick={onSeeSoul}
          title="See your soul"
        >
          <div className="reveal-trend-label">
            Recent sessions <span className="reveal-chev">›</span>
          </div>
          <div className="reveal-trend-row">
            {Array.from({ length: 5 }).map((_, i) => {
              const r = recent[i];
              const lifted = r && r.pre && r.post
                ? (calmScore(r.post.rmssd_ms) ?? 0) - (calmScore(r.pre.rmssd_ms) ?? 0)
                : 0;
              const today = i === 4;
              return (
                <RevealPair
                  key={i}
                  filled={!!r}
                  lifted={lifted}
                  today={today}
                />
              );
            })}
          </div>
          {recent.length >= 2 && (
            <div className="reveal-trend-summary">
              {countLifted(recent)} of {recent.length} lifted you
            </div>
          )}
        </button>

        <div className="reveal-actions">
          <button type="button" className="reveal-btn" onClick={onLearnMore}>
            Learn more
          </button>
          <button type="button" className="reveal-btn reveal-btn-primary" onClick={onDone}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

function countLifted(rs: HrvReading[]): number {
  let n = 0;
  for (const r of rs) {
    if (!r.pre || !r.post) continue;
    const pre = calmScore(r.pre.rmssd_ms) ?? 0;
    const post = calmScore(r.post.rmssd_ms) ?? 0;
    if (post > pre) n++;
  }
  return n;
}

/** One pair-dot column in the micro-trend strip. `lifted > 0` means the
 *  post score was higher than the pre · the post dot sits ABOVE the pre.
 *  `lifted <= 0` means flat or down · post sits NEAR the pre. */
function RevealPair({
  filled,
  lifted,
  today,
}: {
  filled: boolean;
  lifted: number;
  today: boolean;
}) {
  // Map lifted (-20..+20 roughly) to a height in px for visual
  // separation. Clamp so a one-off huge delta does not blow up the row.
  const sep = Math.max(0, Math.min(28, lifted));
  return (
    <div className={`reveal-mark ${today ? "reveal-mark-today" : ""} ${filled ? "" : "reveal-mark-empty"}`}>
      <div className="reveal-post" style={{ marginBottom: sep }} />
      {sep > 0 && <div className="reveal-link" style={{ height: sep - 2 }} />}
      <div className="reveal-pre" />
      {today && <div className="reveal-today">today</div>}
    </div>
  );
}
