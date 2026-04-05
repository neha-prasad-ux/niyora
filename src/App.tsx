import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

const PHASES = ["Inhale", "Hold", "Exhale", "Hold"] as const;
const SECONDS_PER_PHASE = 4;
const TOTAL_ROUNDS = 4;

type Phase = (typeof PHASES)[number];

function App() {
  const [isRunning, setIsRunning] = useState(true);
  const [round, setRound] = useState(1);
  const [phaseIndex, setPhaseIndex] = useState(0);
  const [countdown, setCountdown] = useState(SECONDS_PER_PHASE);
  const [isDone, setIsDone] = useState(false);

  const phase: Phase = PHASES[phaseIndex];

  const resetState = useCallback(() => {
    setRound(1);
    setPhaseIndex(0);
    setCountdown(SECONDS_PER_PHASE);
    setIsDone(false);
    setIsRunning(true);
  }, []);

  // Progress through phases and rounds
  useEffect(() => {
    if (!isRunning || isDone) return;

    const timer = setInterval(() => {
      setCountdown((prev) => {
        if (prev > 1) return prev - 1;

        // Move to next phase
        const nextPhaseIndex = phaseIndex + 1;

        if (nextPhaseIndex >= PHASES.length) {
          // Completed a round
          const nextRound = round + 1;
          if (nextRound > TOTAL_ROUNDS) {
            setIsDone(true);
            setIsRunning(false);
            return 0;
          }
          setRound(nextRound);
          setPhaseIndex(0);
        } else {
          setPhaseIndex(nextPhaseIndex);
        }

        return SECONDS_PER_PHASE;
      });
    }, 1000);

    return () => clearInterval(timer);
  }, [isRunning, isDone, phaseIndex, round]);

  // Reset state when panel is hidden (re-opened from tray starts fresh)
  useEffect(() => {
    const handleVisibility = () => {
      if (document.hidden) {
        resetState();
      }
    };
    document.addEventListener("visibilitychange", handleVisibility);
    return () => document.removeEventListener("visibilitychange", handleVisibility);
  }, [resetState]);

  const handleClose = useCallback(async () => {
    await invoke("hide_panel");
    resetState();
  }, [resetState]);

  // Calculate overall progress (0 to 1)
  const totalPhases = TOTAL_ROUNDS * PHASES.length;
  const completedPhases = (round - 1) * PHASES.length + phaseIndex;
  const phaseProgress = (SECONDS_PER_PHASE - countdown) / SECONDS_PER_PHASE;
  const progress = isDone ? 1 : (completedPhases + phaseProgress) / totalPhases;

  return (
    <div className="container">
      {/* Close X button in top-right corner */}
      <button className="close-x" onClick={handleClose} title="Close">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
          <path d="M2 2l10 10M12 2L2 12" />
        </svg>
      </button>

      <div className="content">
        <h1 className="title">Niyora</h1>

        {isDone ? (
          <div className="done-state">
            <div className="done-icon">&#10003;</div>
            <p className="done-text">Well done. Take this calm with you.</p>
          </div>
        ) : (
          <>
            <div className="breathing-circle">
              <div
                className={`circle ${phase.toLowerCase().replace(" ", "-")}`}
                style={{
                  transform: `scale(${
                    phase === "Inhale"
                      ? 0.6 + 0.4 * phaseProgress
                      : phase === "Exhale"
                      ? 1.0 - 0.4 * phaseProgress
                      : phase === "Hold" && phaseIndex === 1
                      ? 1.0
                      : 0.6
                  })`,
                }}
              >
                <span className="countdown">{countdown}</span>
              </div>
            </div>

            <p className="phase-label">{phase}</p>
          </>
        )}

        {/* Round indicators */}
        <div className="rounds">
          {Array.from({ length: TOTAL_ROUNDS }, (_, i) => (
            <div
              key={i}
              className={`round-dot ${
                i < round - 1
                  ? "completed"
                  : i === round - 1 && !isDone
                  ? "active"
                  : isDone
                  ? "completed"
                  : ""
              }`}
            />
          ))}
        </div>

        {/* Progress bar */}
        <div className="progress-bar">
          <div className="progress-fill" style={{ width: `${progress * 100}%` }} />
        </div>

        <button className="done-button" onClick={handleClose}>
          Done
        </button>
      </div>
    </div>
  );
}

export default App;
