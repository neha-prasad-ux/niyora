import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface SituationalSnapshot {
  score: number;
  interval_min: number;
  label: string;
  contextual_message: string;
  in_meeting: boolean;
  current_app: string | null;
  continuous_screen_min: number;
  cumulative_meeting_min_today: number;
  after_hours: boolean;
}

const POLL_INTERVAL_MS = 30_000;

/**
 * Build a fully-formed snapshot from a single score (0..=100). Used by the
 * dev controls overlay so we can preview every stress tier without waiting
 * for real situational signals.
 */
export function synthesizeSnapshot(score: number): SituationalSnapshot {
  const s = Math.max(0, Math.min(100, score));
  const label = s >= 80 ? "calm" : s >= 60 ? "normal" : s >= 40 ? "dense" : "heavy";
  return {
    score: s,
    interval_min: 90,
    label,
    contextual_message: "Dev override.",
    in_meeting: false,
    current_app: null,
    continuous_screen_min: 0,
    cumulative_meeting_min_today: 0,
    after_hours: false,
  };
}

/**
 * Dev-only: build a synthesised snapshot from URL query params if `force-stress`
 * is present. Returns null otherwise. This entire helper is gated by
 * `import.meta.env.DEV` at the call site so it tree-shakes out of prod builds.
 */
function readForcedSnapshotFromUrl(): SituationalSnapshot | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const forced = params.get("force-stress");
  if (forced === null) return null;
  const parsed = Number(forced);
  if (!Number.isFinite(parsed)) return null;
  const score = Math.max(0, Math.min(100, parsed));

  const deriveLabel = (s: number): string => {
    if (s >= 80) return "calm";
    if (s >= 60) return "normal";
    if (s >= 40) return "dense";
    return "heavy";
  };

  const label = params.get("label") ?? deriveLabel(score);
  const msg = params.get("msg") ?? "Test mode: forced state.";
  const intervalParam = params.get("interval");
  const intervalParsed = intervalParam !== null ? Number(intervalParam) : NaN;
  const interval_min = Number.isFinite(intervalParsed) ? intervalParsed : 90;
  const in_meeting = params.get("meeting") === "true";
  const after_hours = params.get("afterhours") === "true";

  return {
    score,
    interval_min,
    label,
    contextual_message: msg,
    in_meeting,
    current_app: null,
    continuous_screen_min: 0,
    cumulative_meeting_min_today: 0,
    after_hours,
  };
}

export function useSnapshot(): SituationalSnapshot | null {
  // Dev-only URL-param override. The `import.meta.env.DEV` check ensures this
  // entire branch is dropped from production builds by Vite's tree-shaker.
  const forced = import.meta.env.DEV ? readForcedSnapshotFromUrl() : null;

  const [snapshot, setSnapshot] = useState<SituationalSnapshot | null>(forced);

  useEffect(() => {
    // Skip live polling entirely when forced via URL params (dev-only).
    if (import.meta.env.DEV && readForcedSnapshotFromUrl() !== null) {
      return;
    }
    let cancelled = false;
    const fetchSnapshot = async () => {
      try {
        const s = await invoke<SituationalSnapshot>("get_situational_snapshot");
        if (!cancelled) setSnapshot(s);
      } catch {
        /* noop — leave previous snapshot in place */
      }
    };
    fetchSnapshot();
    const interval = setInterval(fetchSnapshot, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  return snapshot;
}

/**
 * Map a 0-100 Niyora Index to a CSS color in the cosmic palette.
 * 100 = near-white indigo (calm), drifting through purple → rose → copper as load grows.
 */
/**
 * Atmospheric drift accents for the stress ball — picks the cloud hue and
 * how visible the clouds should be at each tier. Calm scores get NO clouds
 * (clean, settled) so the orb feels resolved when the user is relaxed.
 */
export interface OrbAccent {
  /** HSL hue for the drifting cloud whorls — matches the body palette. */
  hue: number;
  /** 0..=1.4 multiplier on cloud opacity. 0 = no clouds (calm). */
  strength: number;
}

export function scoreToOrbAccent(score: number): OrbAccent {
  // Hue stays bucketed by tier (each hue is a distinct visual identity).
  // Strength interpolates smoothly with score so crossing a tier boundary
  // doesn't pop the cloud opacity from 0 to 0.55 in one step.
  const hue =
    score >= 80 ? 220 :
    score >= 60 ? 280 :
    score >= 40 ? 330 :
    score >= 20 ? 12  :
                  355;
  const s = Math.max(0, Math.min(100, score));
  // Calm scores (>=80) get strength 0 (no clouds — clean orb).
  // Below 80, ramp up to 1.25 at score 0.
  const t = Math.max(0, (80 - s) / 80);
  const strength = Math.pow(t, 0.7) * 1.25;
  return { hue, strength };
}

export function scoreToColor(score: number): string {
  // Match scoreToBallGradient — cool blue-white at calm, vivid red at overload.
  if (score >= 80) return "hsla(220, 60%, 88%, 0.6)"; // cool blue-white
  if (score >= 60) return "hsla(280, 60%, 65%, 0.6)"; // soft violet
  if (score >= 40) return "hsla(330, 70%, 60%, 0.65)"; // rose pink
  if (score >= 20) return "hsla(12, 80%, 55%, 0.7)"; // coral red
  return "hsla(355, 85%, 52%, 0.75)"; // vivid red
}

/**
 * Halo intensity: 0..1 — barely visible when calm, prominent when stressed.
 * Calm score 100 → 0 (invisible); stressed score 0 → 1 (full strength).
 */
export function scoreToHaloIntensity(score: number): number {
  return Math.max(0, Math.min(1, (100 - score) / 100));
}

/**
 * Color stops for the central gas-planet ball — base radial gradient (highlight,
 * mid, edge) plus two atmospheric-glow tones (inner ring, soft outer haze).
 * White/near-white when calm, drifting through lavender → rose → copper → amber
 * as the score drops.
 */
export interface BallGradient {
  highlight: string;
  mid: string;
  edge: string;
  glow: string;        // tight outer atmospheric ring
  glowFaint: string;   // wider, softer haze beyond the ring
}

export function scoreToBallGradient(score: number): BallGradient {
  // Hue spread cool → warm across the five tiers, with saturation rising
  // as score drops so stressed states feel visibly more intense, not just
  // a different colour. Lightness on the highlight stays bright so the
  // ball always reads as a 3D sphere, not a flat disk.
  if (score >= 80) {
    // Calm — cool blue-white, peaceful
    return {
      highlight: "rgba(255, 255, 255, 0.97)",
      mid: "hsla(220, 25%, 92%, 0.95)",
      edge: "hsla(220, 40%, 72%, 0.9)",
      glow: "hsla(220, 55%, 75%, 0.5)",
      glowFaint: "hsla(220, 50%, 70%, 0.2)",
    };
  }
  if (score >= 60) {
    // Normal — soft violet, gently warming
    return {
      highlight: "rgba(252, 250, 255, 0.96)",
      mid: "hsla(280, 40%, 82%, 0.95)",
      edge: "hsla(280, 55%, 58%, 0.9)",
      glow: "hsla(280, 65%, 62%, 0.55)",
      glowFaint: "hsla(280, 60%, 58%, 0.22)",
    };
  }
  if (score >= 40) {
    // Dense — rose-pink, the body's warming up
    return {
      highlight: "rgba(255, 248, 250, 0.95)",
      mid: "hsla(330, 55%, 78%, 0.95)",
      edge: "hsla(330, 65%, 52%, 0.9)",
      glow: "hsla(330, 75%, 58%, 0.6)",
      glowFaint: "hsla(330, 70%, 52%, 0.25)",
    };
  }
  if (score >= 20) {
    // Heavy — coral red, clearly warm, signals concern
    return {
      highlight: "rgba(255, 238, 232, 0.95)",
      mid: "hsla(12, 65%, 70%, 0.95)",
      edge: "hsla(12, 75%, 48%, 0.9)",
      glow: "hsla(12, 85%, 55%, 0.65)",
      glowFaint: "hsla(12, 80%, 48%, 0.28)",
    };
  }
  // Overload — vivid red, urgent
  return {
    highlight: "rgba(255, 230, 226, 0.95)",
    mid: "hsla(355, 70%, 65%, 0.95)",
    edge: "hsla(355, 80%, 45%, 0.9)",
    glow: "hsla(355, 90%, 50%, 0.7)",
    glowFaint: "hsla(355, 85%, 45%, 0.3)",
  };
}
