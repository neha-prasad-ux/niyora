import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { scoreToBallGradient } from "./useSnapshot";

interface Props {
  onDone: () => void;
}

const DOT_LABELS = ["Tense", "Heavy", "Neutral", "Settled", "Calm"];

export default function PostSessionMood({ onDone }: Props) {
  const [submitting, setSubmitting] = useState(false);

  const handleTap = useCallback(
    async (value: number | null) => {
      if (submitting) return;
      setSubmitting(true);
      if (value !== null) {
        try {
          await invoke("log_event", {
            eventType: "mood_post",
            meta: { value },
          });
        } catch {
          /* silent — mood capture is best-effort */
        }
      }
      onDone();
    },
    [onDone, submitting]
  );

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
        <div className="mood-eyebrow">After breath</div>
        <div className="mood-question">How do you feel?</div>
        <div className="mood-dots">
          {[1, 2, 3, 4, 5].map((v) => (
            <button
              key={v}
              className={`mood-dot mood-dot-${v}`}
              onClick={() => handleTap(v)}
              disabled={submitting}
              aria-label={DOT_LABELS[v - 1]}
              title={DOT_LABELS[v - 1]}
            />
          ))}
        </div>
        <button
          className="mood-skip"
          onClick={() => handleTap(null)}
          disabled={submitting}
        >
          Skip
        </button>
      </div>
    </div>
  );
}
