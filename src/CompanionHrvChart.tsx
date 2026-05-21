import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { calmScore, calmLabel } from "./calmScore";

interface HrvReading {
  received_at: string;
  session_id: string;
  pre_ms: number | null;
  post_ms: number | null;
  delta_ms: number | null;
  samples_pre: number;
  samples_post: number;
  status: string;
}

/** Trend chart of calm scores derived from the iPhone's HRV readings.
 * Lives inside the Connect-your-iPhone card; renders only when there is
 * at least one ok-status reading on file. */
export function CompanionHrvChart() {
  const [readings, setReadings] = useState<HrvReading[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      invoke<HrvReading[]>("companion_hrv_history")
        .then((rs) => {
          if (cancelled) return;
          setReadings(rs);
          setLoaded(true);
        })
        .catch(() => {
          if (cancelled) return;
          setReadings([]);
          setLoaded(true);
        });
    };
    refresh();
    const unlisten = listen("companion://state", () => refresh());
    return () => {
      cancelled = true;
      unlisten.then((u) => u()).catch(() => {});
    };
  }, []);

  if (!loaded) return null;

  // Keep only readings with a usable post_ms · "no data" rows from the
  // sparse-Watch path don't move the trend and shouldn't render as dots.
  const points = readings
    .filter((r) => r.status === "ok" && typeof r.post_ms === "number")
    .map((r) => ({
      at: Date.parse(r.received_at),
      hrvMs: r.post_ms as number,
      score: calmScore(r.post_ms) as number,
      delta: r.delta_ms,
    }))
    .filter((p) => Number.isFinite(p.at))
    .sort((a, b) => a.at - b.at);

  if (points.length === 0) return null;

  const latest = points[points.length - 1];
  const avg =
    points.reduce((s, p) => s + p.score, 0) / points.length;

  // 30-day window starting from min(first reading, 30 days ago).
  const now = Date.now();
  const windowStart = Math.min(
    points[0].at,
    now - 30 * 24 * 3600 * 1000,
  );
  const span = Math.max(now - windowStart, 24 * 3600 * 1000);

  const W = 280;
  const H = 90;
  const PAD_X = 8;
  const PAD_Y = 8;

  const xFor = (t: number) =>
    PAD_X + ((t - windowStart) / span) * (W - PAD_X * 2);
  const yFor = (score: number) =>
    PAD_Y + (1 - score / 100) * (H - PAD_Y * 2);

  const path = points
    .map((p, i) => `${i === 0 ? "M" : "L"}${xFor(p.at).toFixed(1)},${yFor(p.score).toFixed(1)}`)
    .join(" ");

  return (
    <div className="companion-hrv-trend">
      <div className="companion-hrv-headline">
        <div className="companion-hrv-score">{latest.score}</div>
        <div className="companion-hrv-meta">
          <div className="companion-hrv-label">{calmLabel(latest.score)}</div>
          <div className="companion-hrv-sub">
            HRV {latest.hrvMs.toFixed(0)} ms · avg this period {Math.round(avg)}
          </div>
        </div>
      </div>
      <svg
        className="companion-hrv-chart"
        viewBox={`0 0 ${W} ${H}`}
        width={W}
        height={H}
      >
        {/* Calm/neutral/activated bands as background, so a single dot is readable in context */}
        <rect x={0} y={yFor(100)} width={W} height={yFor(60) - yFor(100)}
              fill="hsla(140, 50%, 50%, 0.06)" />
        <rect x={0} y={yFor(60)} width={W} height={yFor(40) - yFor(60)}
              fill="hsla(60, 50%, 50%, 0.04)" />
        <rect x={0} y={yFor(40)} width={W} height={yFor(0) - yFor(40)}
              fill="hsla(0, 50%, 50%, 0.05)" />

        {points.length > 1 && (
          <path
            d={path}
            fill="none"
            stroke="hsla(265, 60%, 70%, 0.75)"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        )}
        {points.map((p, i) => (
          <circle
            key={i}
            cx={xFor(p.at)}
            cy={yFor(p.score)}
            r={i === points.length - 1 ? 3.5 : 2.2}
            fill={
              i === points.length - 1
                ? "hsla(265, 75%, 80%, 1)"
                : "hsla(265, 50%, 65%, 0.85)"
            }
          />
        ))}
      </svg>
      <div className="companion-hrv-footer">
        {points.length} {points.length === 1 ? "reading" : "readings"} · take
        these gently, single numbers are noisy.
      </div>
    </div>
  );
}
