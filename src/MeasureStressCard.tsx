import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { calmScore } from "./calmScore";

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
interface PairedDevice {
  client_id: string;
  client_name: string;
  paired_at: string;
  last_seen: string | null;
  last_window_sent_at: string | null;
}
interface CompanionStatus {
  paired_devices: PairedDevice[];
}

type Phase = "pre" | "post";

interface Props {
  /** Session this card is binding measurements to. */
  sessionId: string;
  /** Which side of the session this card captures. */
  phase: Phase;
  /** Technique name forwarded to the iPhone for context on its sheet. */
  techniqueName: string;
  /** Optional · invoked when the user taps the pair CTA in the
   *  not-paired state so the host view can open the pairing flow. */
  onRequestPair?: () => void;
  /** Optional · invoked the first time the user taps Measure for this
   *  session+phase. Lets the host view defer its auto-dismiss timer
   *  while the iPhone is capturing. */
  onMeasureStarted?: () => void;
}

type State =
  | { kind: "not_paired" }
  | { kind: "ready"; lastDeltaPct: number | null }
  | { kind: "measuring"; startedAt: number }
  | { kind: "result_ok"; deltaPct: number }
  | { kind: "result_low_signal" };

const MEASURE_DURATION_MS = 30_000;

/**
 * State-machine card for the "Measure stress" flow. Detects whether an
 * iPhone is paired, surfaces a Measure CTA, animates a 30s countdown
 * while the phone captures, then shows the result. Same component is
 * used on the pre-session info screen and on the post-session mood
 * screen, parameterised by `phase`.
 */
export default function MeasureStressCard({
  sessionId,
  phase,
  techniqueName,
  onRequestPair,
  onMeasureStarted,
}: Props) {
  const [paired, setPaired] = useState<boolean | null>(null);
  const [readings, setReadings] = useState<HrvReading[]>([]);
  const [measureStartedAt, setMeasureStartedAt] = useState<number | null>(null);
  const tickRef = useRef<number | null>(null);
  const [, forceTick] = useState(0);

  // Initial fetch + state-event subscription. companion_status and the
  // history both refresh whenever the companion sync emits a change ·
  // pair, unpair, new reading, etc.
  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      invoke<CompanionStatus>("companion_status")
        .then((s) => {
          if (cancelled) return;
          setPaired((s.paired_devices?.length ?? 0) > 0);
        })
        .catch(() => {
          if (cancelled) return;
          setPaired(false);
        });
      invoke<HrvReading[]>("companion_hrv_history")
        .then((rs) => {
          if (cancelled) return;
          setReadings(rs);
        })
        .catch(() => {
          /* best effort */
        });
    };
    refresh();
    const unlisten = listen("companion://state", refresh);
    return () => {
      cancelled = true;
      unlisten.then((u) => u()).catch(() => {});
    };
  }, []);

  // Live countdown tick · 250 ms is fast enough to look smooth and slow
  // enough to be cheap. Stopped when no measurement is in flight.
  useEffect(() => {
    if (measureStartedAt === null) {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
      return;
    }
    tickRef.current = window.setInterval(() => {
      forceTick((n) => n + 1);
    }, 250);
    return () => {
      if (tickRef.current !== null) {
        window.clearInterval(tickRef.current);
        tickRef.current = null;
      }
    };
  }, [measureStartedAt]);

  /** This session's row, if the phone has sent anything for it. */
  const currentRow = useMemo(
    () => readings.find((r) => r.session_id === sessionId) ?? null,
    [readings, sessionId],
  );

  /** Most recent ok reading not from this session · drives the "last
   *  reading lifted you X%" subtitle on the ready state. */
  const latestPriorOk = useMemo(() => {
    const others = readings
      .filter((r) => r.session_id !== sessionId)
      .filter((r) => r.pre?.status === "ok" && r.post?.status === "ok")
      .slice()
      .sort((a, b) => Date.parse(b.first_seen_at) - Date.parse(a.first_seen_at));
    return others[0] ?? null;
  }, [readings, sessionId]);

  /** Derive the current state from the data + the in-flight flag. */
  const state: State = useMemo(() => {
    if (paired === null) return { kind: "ready", lastDeltaPct: null };
    if (paired === false) return { kind: "not_paired" };

    const currentCapture = currentRow ? currentRow[phase] : null;
    if (currentCapture) {
      if (currentCapture.status === "ok" && currentRow?.pre && currentRow?.post) {
        const pre = calmScore(currentRow.pre.rmssd_ms);
        const post = calmScore(currentRow.post.rmssd_ms);
        if (pre !== null && post !== null) {
          return { kind: "result_ok", deltaPct: Math.round(post - pre) };
        }
      }
      if (currentCapture.status === "low_signal") {
        return { kind: "result_low_signal" };
      }
    }

    if (measureStartedAt !== null) {
      const elapsed = Date.now() - measureStartedAt;
      if (elapsed < MEASURE_DURATION_MS + 5_000) {
        return { kind: "measuring", startedAt: measureStartedAt };
      }
    }

    const lastDeltaPct = latestPriorOk
      ? (() => {
          const pre = calmScore(latestPriorOk.pre?.rmssd_ms ?? null);
          const post = calmScore(latestPriorOk.post?.rmssd_ms ?? null);
          if (pre === null || post === null) return null;
          return Math.round(post - pre);
        })()
      : null;
    return { kind: "ready", lastDeltaPct };
  }, [paired, currentRow, phase, measureStartedAt, latestPriorOk]);

  const startMeasurement = useCallback(() => {
    if (state.kind === "measuring") return;
    setMeasureStartedAt(Date.now());
    onMeasureStarted?.();
    invoke("companion_request_measurement", {
      sessionId,
      phase,
      techniqueName,
    }).catch(() => {
      /* best-effort · phone may be offline. The state machine will
         time out the measuring state and revert to ready. */
    });
  }, [state.kind, sessionId, phase, techniqueName, onMeasureStarted]);

  // Hide the card entirely while we don't yet know if a phone is paired ·
  // avoids a flash of "pair your iPhone" right when the panel opens.
  if (paired === null) return null;

  return (
    <div className={`measure-card measure-card-${state.kind}`}>
      <div className="measure-dot" />
      <div className="measure-body">{renderBody(state, phase)}</div>
      <div className="measure-cta">
        {renderCta(state, phase, startMeasurement, onRequestPair)}
      </div>
    </div>
  );
}

// MARK: - Renderers per state.

function renderBody(state: State, phase: Phase) {
  switch (state.kind) {
    case "not_paired":
      return (
        <>
          <div className="measure-title">Measure stress</div>
          <div className="measure-sub">
            Pair your iPhone. 30 second reading from the camera, never
            leaves your devices.
          </div>
        </>
      );
    case "ready":
      return (
        <>
          <div className="measure-title">Measure stress</div>
          <div className="measure-sub">
            {state.lastDeltaPct !== null
              ? `Your last session lifted you ${state.lastDeltaPct}%.`
              : phase === "pre"
                ? "30 second reading from your iPhone before you breathe."
                : "30 second reading from your iPhone."}
          </div>
        </>
      );
    case "measuring": {
      const elapsedMs = Math.max(0, Date.now() - state.startedAt);
      const remaining = Math.max(0, MEASURE_DURATION_MS - elapsedMs);
      const remainingSec = Math.ceil(remaining / 1000);
      const stage =
        elapsedMs < 4_000
          ? "Open Niyora on your iPhone."
          : elapsedMs < 8_000
            ? "Place your fingertip over the back camera and flashlight."
            : `Measuring. ${remainingSec}s left.`;
      return (
        <>
          <div className="measure-title">Hold still</div>
          <div className="measure-sub">{stage}</div>
        </>
      );
    }
    case "result_ok":
      return (
        <>
          <div className="measure-title">
            {state.deltaPct >= 0
              ? `Up ${state.deltaPct}% calmer`
              : `Down ${Math.abs(state.deltaPct)}%`}
          </div>
          <div className="measure-sub">
            {phase === "pre"
              ? "Now breathe to settle further."
              : "Breath worked."}
          </div>
        </>
      );
    case "result_low_signal":
      return (
        <>
          <div className="measure-title">Reading was too noisy</div>
          <div className="measure-sub">
            Press your fingertip more firmly over the lens and try again.
          </div>
        </>
      );
  }
}

function renderCta(
  state: State,
  _phase: Phase,
  startMeasurement: () => void,
  onRequestPair?: () => void,
) {
  switch (state.kind) {
    case "not_paired":
      return (
        <button
          type="button"
          className="measure-btn"
          onClick={onRequestPair}
          disabled={!onRequestPair}
        >
          Pair iPhone
        </button>
      );
    case "ready":
      return (
        <button
          type="button"
          className="measure-btn measure-btn-primary"
          onClick={startMeasurement}
        >
          Measure
        </button>
      );
    case "measuring": {
      const elapsedMs = Math.max(0, Date.now() - state.startedAt);
      const progress = Math.min(1, elapsedMs / MEASURE_DURATION_MS);
      return (
        <div
          className="measure-progress"
          aria-label={`Measuring, ${Math.round(progress * 100)} percent done`}
        >
          <div
            className="measure-progress-fill"
            style={{ width: `${progress * 100}%` }}
          />
        </div>
      );
    }
    case "result_ok":
      return null;
    case "result_low_signal":
      return (
        <button
          type="button"
          className="measure-btn"
          onClick={startMeasurement}
        >
          Try again
        </button>
      );
  }
}
