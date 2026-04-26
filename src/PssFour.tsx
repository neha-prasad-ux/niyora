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

export default function PssFour({ onDone }: Props) {
  const [step, setStep] = useState(0);
  const [answers, setAnswers] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);

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
        await invoke("submit_pss4", { answers: next });
      } catch {
        /* still close */
      }
      onDone();
    },
    [answers, step, busy, onDone]
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
