import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onDone: () => void;
}

const QUESTIONS = [
  "How often have you felt unable to control the important things in your life?",
  "How often have you felt confident about your ability to handle your personal problems?",
  "How often have you felt that things were going your way?",
  "How often have you felt difficulties were piling up so high that you could not overcome them?",
];

const SCALE = ["Never", "Almost never", "Sometimes", "Fairly often", "Very often"];

type Phase = "questions" | "result";

interface Band {
  label: string;
  message: string;
  hue: number;
}

/** Map a 0..=16 PSS-4 score to the app's calm → heavy vocabulary. */
function bandFor(score: number): Band {
  if (score <= 4)  return { label: "Calm",   message: "You're carrying this lightly right now.", hue: 250 };
  if (score <= 8)  return { label: "Normal", message: "A typical week.",                          hue: 265 };
  if (score <= 12) return { label: "Dense",  message: "More weight than usual.",                  hue: 335 };
  return             { label: "Heavy",  message: "A lot is sitting on you right now.",       hue: 20  };
}

export default function PssFour({ onDone }: Props) {
  const [phase, setPhase] = useState<Phase>("questions");
  const [step, setStep] = useState(0);
  const [answers, setAnswers] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const [score, setScore] = useState<number | null>(null);

  const handleAnswer = useCallback(
    async (value: number) => {
      if (busy) return;
      const next = [...answers, value];
      if (step < 3) {
        setAnswers(next);
        setStep(step + 1);
        return;
      }
      setBusy(true);
      try {
        const total = await invoke<number>("submit_pss4", { answers: next });
        setScore(total);
      } catch {
        setScore(null);
      }
      setBusy(false);
      setPhase("result");
    },
    [answers, step, busy]
  );

  const handleSkip = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    try {
      await invoke("dismiss_pss4");
    } catch {
      /* ignore */
    }
    onDone();
  }, [busy, onDone]);

  if (phase === "result") {
    const shown = score ?? 0;
    const band = bandFor(shown);
    const accent = `hsla(${band.hue}, 60%, 70%, 1)`;
    return (
      <div className="niyora-pss4">
        <div className="pss4-backdrop" />
        <div className="pss4-content pss4-result">
          <div className="pss4-eyebrow">Your check-in</div>
          <div className="pss4-score-wrap">
            <div className="pss4-score-num" style={{ color: accent }}>{shown}</div>
            <div className="pss4-score-out">out of 16</div>
          </div>
          <div className="pss4-band" style={{ color: accent }}>{band.label}</div>
          <div className="pss4-band-msg">{band.message}</div>
          <div className="pss4-result-note">
            Take this every once in a while to see how you feel over time.
            Your scores stay on this Mac.
          </div>
          <button className="pss4-result-btn" onClick={onDone}>See history</button>
        </div>
      </div>
    );
  }

  return (
    <div className="niyora-pss4">
      <div className="pss4-backdrop" />
      <div className="pss4-content">
        <button
          className="pss4-back"
          onClick={onDone}
          disabled={busy}
          title="Back"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
        <div className="pss4-eyebrow">
          Weekly check-in · {step + 1} of 4
        </div>
        <div className="pss4-prefix">In the last week,</div>
        <div className="pss4-question">{QUESTIONS[step]}</div>
        <div className="pss4-options">
          {SCALE.map((label, i) => (
            <button
              key={i}
              className="pss4-option"
              onClick={() => handleAnswer(i)}
              disabled={busy}
            >
              {label}
            </button>
          ))}
        </div>
        <button className="pss4-skip" onClick={handleSkip} disabled={busy}>
          Skip this week
        </button>
      </div>
    </div>
  );
}
