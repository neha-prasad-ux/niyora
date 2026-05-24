import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useSnapshot, scoreToBallGradient } from "./useSnapshot";
import { useSessionStats } from "./useSessionStats";
import { TIERS, currentTier, nextTier, sessionsToNext, type Tier } from "./tiers";
import { CompanionCard } from "./CompanionCard";

interface Props {
  onBack: () => void;
  onOpenPss4: () => void;
}

interface Pss4Entry {
  date: string;
  score: number;
}

const LABEL_TEXT: Record<string, string> = {
  calm: "Calm",
  normal: "Normal",
  dense: "Dense",
  heavy: "Heavy",
};

/** Hue accent per tier — warm → cool as the soul brightens. */
function tierAccentHue(tier: Tier): number {
  switch (tier.id) {
    case "spark":      return 30;
    case "glow":       return 335;
    case "shine":      return 280;
    case "radiance":   return 230;
    case "brilliance": return 210;
  }
}

/** Number of Saturn-style rings to render for a tier. */
function tierRingCount(tier: Tier): number {
  switch (tier.id) {
    case "spark":      return 0;
    case "glow":       return 1;
    case "shine":      return 2;
    case "radiance":   return 3;
    case "brilliance": return 4;
  }
}

/** Dev-only: ?force-sessions=N simulates a session count for tier testing. */
function readForcedSessions(): number | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const forced = params.get("force-sessions");
  if (forced === null) return null;
  const n = Number(forced);
  return Number.isFinite(n) ? n : null;
}

/** PSS-4 sparkline. SVG, 0..=16 score range, oldest left → newest right.
 * Defensively filters out entries with non-numeric scores so a malformed
 * line in events.jsonl can't render NaN coordinates and break the SVG. */
function Pss4Sparkline({ history }: { history: Pss4Entry[] }) {
  const valid = history.filter(
    (e) => Number.isFinite(e.score) && e.score >= 0 && e.score <= 16
  );
  if (valid.length === 0) return null;
  const W = 240;
  const H = 50;
  const PAD = 6;
  const max = 16;
  const points = valid.map((e, i) => {
    const x = valid.length === 1
      ? W / 2
      : PAD + (i / (valid.length - 1)) * (W - PAD * 2);
    const y = PAD + (1 - e.score / max) * (H - PAD * 2);
    return { x, y, score: e.score };
  });
  const path = points
    .map((p, i) => `${i === 0 ? "M" : "L"}${p.x.toFixed(1)},${p.y.toFixed(1)}`)
    .join(" ");
  const last = points[points.length - 1];
  return (
    <svg className="soul-spark" viewBox={`0 0 ${W} ${H}`} width={W} height={H}>
      <line x1={PAD} y1={H / 2} x2={W - PAD} y2={H / 2} stroke="rgba(255,255,255,0.06)" strokeWidth="1" />
      {points.length > 1 && (
        <path d={path} fill="none" stroke="hsla(265, 60%, 70%, 0.85)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      )}
      {points.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r={i === points.length - 1 ? 3 : 2}
          fill={i === points.length - 1 ? "hsla(265, 70%, 80%, 1)" : "hsla(265, 50%, 60%, 0.7)"} />
      ))}
      <text x={last.x} y={Math.max(12, last.y - 6)} textAnchor="middle"
        fontSize="10" fill="rgba(255,255,255,0.85)" fontFamily="Poppins, sans-serif">
        {last.score}
      </text>
    </svg>
  );
}

export default function Settings({ onBack, onOpenPss4 }: Props) {
  const snapshot = useSnapshot();
  const liveStats = useSessionStats();
  const [forcedSessions, setForcedSessions] = useState(() =>
    import.meta.env.DEV ? readForcedSessions() : null
  );
  const completed = forcedSessions ?? liveStats.completed;

  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion().then(setVersion).catch(() => {
      /* leave empty if the API is unavailable in dev preview */
    });
  }, []);

  // Launch at login: defaults to true for new installs.
  const [launchAtLogin, setLaunchAtLogin] = useState<boolean | null>(null);
  useEffect(() => {
    invoke<boolean>("get_launch_at_login")
      .then(setLaunchAtLogin)
      .catch(() => setLaunchAtLogin(null));
  }, []);
  const toggleLaunchAtLogin = (next: boolean) => {
    setLaunchAtLogin(next);
    invoke("set_launch_at_login", { enabled: next }).catch(() => {
      setLaunchAtLogin(!next);
    });
  };

  // Analytics consent: `null` while loading or if the backend is unavailable,
  // then `true` / `false` per the onboarding choice. Saved immediately on toggle.
  const [analyticsOn, setAnalyticsOn] = useState<boolean | null>(null);
  useEffect(() => {
    invoke<boolean | null>("analytics_consent_status")
      .then((v) => setAnalyticsOn(v))
      .catch(() => setAnalyticsOn(null));
  }, []);
  const toggleAnalytics = (next: boolean) => {
    setAnalyticsOn(next);
    invoke("set_analytics_consent", { granted: next }).catch(() => {
      /* revert on failure so UI matches backend state */
      setAnalyticsOn(!next);
    });
  };

  const [pss4History, setPss4History] = useState<Pss4Entry[]>([]);
  useEffect(() => {
    let cancelled = false;
    invoke<Pss4Entry[]>("pss4_history")
      .then((h) => {
        if (!cancelled) setPss4History(h);
      })
      .catch(() => {
        /* leave empty */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const score = snapshot?.score ?? 100;
  const label = snapshot?.label ?? "calm";
  const grad = scoreToBallGradient(score);

  const tier = currentTier(completed);
  const next = nextTier(tier);
  const remaining = sessionsToNext(completed);
  const accentHue = tierAccentHue(tier);
  const rings = tierRingCount(tier);

  const stressLine = `Today: ${LABEL_TEXT[label] ?? label}`;

  return (
    <div className="niyora-soul">
      <div className="soul-backdrop" />
      <div className="soul-content">
        <button className="soul-close" onClick={onBack} title="Back" aria-label="Back">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>

        <div className="soul-header">My Soul</div>

        <div className="soul-planet-wrap">
          <div
            className="soul-planet"
            style={{
              background: `radial-gradient(circle at 35% 30%, ${grad.highlight} 0%, ${grad.mid} 45%, ${grad.edge} 100%)`,
              boxShadow: `
                0 0 28px 4px ${grad.glow},
                0 0 64px 16px ${grad.glowFaint},
                0 14px 36px rgba(0, 0, 0, 0.35),
                inset -22px -18px 36px rgba(0, 0, 0, 0.32),
                inset 14px 10px 26px rgba(255, 255, 255, 0.10)
              `,
            }}
          />
          {Array.from({ length: rings }).map((_, i) => (
            <div
              key={i}
              className="soul-ring"
              style={{
                width: 200 + i * 18,
                height: 28 + i * 4,
                borderColor: `hsla(${accentHue}, 70%, ${65 - i * 5}%, ${0.55 - i * 0.08})`,
                boxShadow: `0 0 12px hsla(${accentHue}, 70%, 60%, ${0.25 - i * 0.04})`,
              }}
            />
          ))}
        </div>

        <div className="soul-stress">{stressLine}</div>

        {/* Sessions + tier card with hierarchy: Level X up top, "N to Y" subtle */}
        <div className="soul-card soul-card-progress">
          <div className="soul-tier-headline">
            <span className="soul-tier-name" style={{ color: `hsla(${accentHue}, 70%, 75%, 1)` }}>
              Level {tier.name}
            </span>
            {next && (
              <span className="soul-tier-sub">
                {remaining} to {next.name}
              </span>
            )}
          </div>

          <div className="soul-sessions-row">
            <span className="soul-sessions-num">{completed}</span>
            <span className="soul-sessions-label">sessions</span>
          </div>

          <div className="soul-tier-track">
            <div
              className="soul-tier-track-fill"
              style={{
                width: `${Math.min(100, (completed / 80) * 100)}%`,
                background: `linear-gradient(90deg, hsla(${accentHue}, 70%, 60%, 0.6), hsla(${accentHue}, 80%, 70%, 0.9))`,
              }}
            />
            {TIERS.map((t) => {
              // Skip the Spark marker (threshold 0) — it's always reached
              // and the line's gradient already shows the starting point.
              if (t.threshold === 0) return null;
              const reached = completed >= t.threshold;
              const leftPct = (t.threshold / 80) * 100;
              return (
                <div
                  key={t.id}
                  className={`soul-tier-marker ${reached ? "is-reached" : ""}`}
                  style={{
                    left: `${leftPct}%`,
                    borderColor: reached
                      ? `hsla(${accentHue}, 50%, 65%, 0.85)`
                      : "rgba(255,255,255,0.22)",
                    background: reached
                      ? `hsla(${accentHue}, 50%, 25%, 0.85)`
                      : "rgba(20, 18, 30, 0.95)",
                  }}
                  title={`${t.name} · ${t.threshold === 0 ? "start" : `${t.threshold} sessions`}`}
                >
                  <span className="soul-tier-marker-num">
                    {t.threshold === 0 ? "" : t.threshold}
                  </span>
                </div>
              );
            })}
          </div>
        </div>

        {/* PSS-4 mental health card with history sparkline */}
        <div className="soul-card soul-card-cta">
          <div className="soul-cta-title">Mental health check-in</div>
          <div className="soul-cta-blurb">
            {pss4History.length === 0
              ? "Take the 1-minute test. Your scores stay with you."
              : `${pss4History.length} check-in${pss4History.length === 1 ? "" : "s"} so far. Lower is calmer.`}
          </div>
          {pss4History.length > 0 && <Pss4Sparkline history={pss4History} />}
          <button className="soul-cta-btn" onClick={onOpenPss4}>
            {pss4History.length === 0 ? "Begin" : "Take again"}
          </button>
        </div>

        {launchAtLogin !== null && (
          <div className="soul-card soul-card-toggle">
            <div className="soul-toggle-row">
              <div className="soul-toggle-text">
                <div className="soul-cta-title">Launch at login</div>
                <div className="soul-cta-blurb">
                  Start Niyora when your Mac starts so reminders are always ready.
                </div>
              </div>
              <label className="soul-switch" aria-label="Toggle launch at login">
                <input
                  type="checkbox"
                  checked={launchAtLogin}
                  onChange={(e) => toggleLaunchAtLogin(e.target.checked)}
                />
                <span className="soul-switch-track" />
              </label>
            </div>
          </div>
        )}

        <div className="soul-card soul-card-toggle">
          <div className="soul-toggle-row">
            <div className="soul-toggle-text">
              <div className="soul-cta-title">Anonymous analytics</div>
              <div className="soul-cta-blurb">
                Helps shape what to improve next. Stress scores, breath patterns, and anything that identifies you stay on your Mac.
              </div>
            </div>
            <label className="soul-switch" aria-label="Toggle anonymous analytics">
              <input
                type="checkbox"
                checked={analyticsOn === true}
                onChange={(e) => toggleAnalytics(e.target.checked)}
              />
              <span className="soul-switch-track" />
            </label>
          </div>
        </div>

        <div className="soul-card soul-card-cta">
          <div className="soul-cta-title">Message the founder</div>
          <div className="soul-cta-blurb">
            Tell Neha what's working, what isn't, what you'd love next.
          </div>
          <button
            className="soul-cta-btn"
            onClick={() => {
              openUrl("https://niyora.com/talk").catch(() => {
                /* opener plugin unavailable; nothing useful to do */
              });
            }}
          >
            Open
          </button>
        </div>

        <CompanionCard />

        <div className="soul-footer">
          Niyora runs entirely on your Mac. No accounts, no profiles. Analytics are anonymous, optional, and only sent if you choose. Breathe easy.
          {version && <span className="soul-version">Version {version}</span>}
        </div>

        {import.meta.env.DEV && (
          <div className="soul-dev-tiers">
            <div className="soul-dev-label">DEV · Tier preview</div>
            <div className="soul-dev-row">
              {TIERS.map((t) => (
                <button
                  key={t.id}
                  className={`soul-dev-btn ${forcedSessions === t.threshold ? "is-current" : ""}`}
                  onClick={() => setForcedSessions(t.threshold)}
                  title={`${t.name} (${t.threshold})`}
                >
                  {t.name}
                </button>
              ))}
              <button
                className={`soul-dev-btn ${forcedSessions === null ? "is-current" : ""}`}
                onClick={() => setForcedSessions(null)}
                title="Use real session count"
              >
                Real
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
