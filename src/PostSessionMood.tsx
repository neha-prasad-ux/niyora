import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { scoreToBallGradient } from "./useSnapshot";
import PostSessionReveal from "./PostSessionReveal";

interface Props {
  onDone: () => void;
  /** Bound to the session that just finished, so a post-tap captures into
   *  the same history row as any pre-session capture. */
  sessionId: string;
  /** Open My Soul · invoked when the user taps the trend strip in the
   *  reveal screen. */
  onSeeSoul: () => void;
}

const DOT_LABELS = ["Tense", "Heavy", "Neutral", "Settled", "Calm"];

interface PhaseStatus {
  status: string;
}
interface HrvReading {
  session_id: string;
  pre: PhaseStatus | null;
  post: PhaseStatus | null;
}

type Phase = "ask" | "thanks" | "reveal";

export default function PostSessionMood({ onDone, sessionId, onSeeSoul }: Props) {
  const [phase, setPhase] = useState<Phase>("ask");
  const [companionPaired, setCompanionPaired] = useState(false);
  const [postPulseSent, setPostPulseSent] = useState(false);
  const [hasBothCaptures, setHasBothCaptures] = useState(false);

  // We deliberately do not store the user's answer. The dots exist as a
  // moment of self-reflection, nothing more. The closing screen is the
  // acknowledgment.
  const handleTap = useCallback(() => {
    if (phase !== "ask") return;
    setPhase("thanks");
  }, [phase]);

  useEffect(() => {
    interface MinStatus { paired_devices?: { client_id: string }[] }
    invoke<MinStatus>("companion_status")
      .then((s) => setCompanionPaired((s.paired_devices?.length ?? 0) > 0))
      .catch(() => setCompanionPaired(false));
  }, []);

  // Subscribe to companion history updates. When both pre and post land
  // for this session, swap from the mood-dots view to the reveal view ·
  // the moment is the payoff for taking both measurements.
  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      invoke<HrvReading[]>("companion_hrv_history")
        .then((rs) => {
          if (cancelled) return;
          const r = rs.find((row) => row.session_id === sessionId);
          if (
            r &&
            r.pre?.status === "ok" &&
            r.post?.status === "ok"
          ) {
            setHasBothCaptures(true);
            setPhase("reveal");
          }
        })
        .catch(() => {
          /* best-effort */
        });
    };
    refresh();
    const unlisten = listen("companion://state", refresh);
    return () => {
      cancelled = true;
      unlisten.then((u) => u()).catch(() => {});
    };
  }, [sessionId]);

  const requestPostMeasurement = useCallback(() => {
    if (postPulseSent) return;
    setPostPulseSent(true);
    invoke("companion_request_measurement", {
      sessionId,
      phase: "post",
      // technique_name is informational on the phone (shown on the
      // measurement sheet). The mood screen doesn't know it locally;
      // sending an empty string is fine · the phone falls back to a
      // generic label.
      techniqueName: "",
    }).catch(() => {
      /* best-effort; phone may be offline */
    });
  }, [postPulseSent, sessionId]);

  useEffect(() => {
    if (phase !== "thanks") return;
    // If the user tapped Measure stress and the post result has not yet
    // arrived, keep the panel open so the reveal can take over once both
    // captures land. The reveal effect above will swap phase from
    // "thanks" to "reveal" when that happens, and this effect's cleanup
    // cancels any pending dismissal.
    if (postPulseSent && !hasBothCaptures) return;
    const t = setTimeout(onDone, 2800);
    return () => clearTimeout(t);
  }, [phase, onDone, postPulseSent, hasBothCaptures]);

  // Reveal flow takes over the panel · its own Done button drives the
  // dismissal, the mood-dots auto-dismiss timer doesn't apply.
  if (phase === "reveal" && hasBothCaptures) {
    return (
      <PostSessionReveal
        sessionId={sessionId}
        onDone={onDone}
        onLearnMore={onDone}
        onSeeSoul={onSeeSoul}
      />
    );
  }

  // Always the calm-tier orb. The "after" ball is the visual reward — it
  // signals "you brought yourself to a settled state" regardless of the
  // longer-term Niyora Index.
  const calm = scoreToBallGradient(100);

  return (
    <div className="niyora-mood">
      <div className="mood-backdrop" />
      <div className="mood-content">
        <div
          className="mood-orb"
          style={{
            background: `radial-gradient(circle at 35% 30%, ${calm.highlight} 0%, ${calm.mid} 45%, ${calm.edge} 100%)`,
            boxShadow: `
              0 0 32px 6px ${calm.glow},
              0 0 72px 18px ${calm.glowFaint},
              0 14px 40px rgba(0, 0, 0, 0.35),
              inset -22px -18px 36px rgba(0, 0, 0, 0.32),
              inset 14px 10px 26px rgba(255, 255, 255, 0.10)
            `,
          }}
        />
        {phase === "ask" ? (
          <>
            <div className="mood-eyebrow">After breath</div>
            <div className="mood-question">How does this feel?</div>
            <div className="mood-dots">
              {[1, 2, 3, 4, 5].map((v) => (
                <div key={v} className="mood-dot-wrap">
                  <button
                    className={`mood-dot mood-dot-${v}`}
                    onClick={handleTap}
                    aria-label={DOT_LABELS[v - 1]}
                    title={DOT_LABELS[v - 1]}
                  />
                  <span className="mood-dot-label">{DOT_LABELS[v - 1]}</span>
                </div>
              ))}
            </div>
            {companionPaired && (
              <button
                className="mood-secondary"
                onClick={requestPostMeasurement}
                disabled={postPulseSent}
              >
                {postPulseSent ? "Sent to your iPhone" : "Measure stress"}
              </button>
            )}
            <button className="mood-skip" onClick={handleTap}>
              Skip
            </button>
          </>
        ) : (
          <div className="mood-thanks">
            <div className="mood-thanks-line">Have a good time.</div>
            <div className="mood-thanks-sub">Come back to feel calm.</div>
          </div>
        )}
      </div>
    </div>
  );
}
