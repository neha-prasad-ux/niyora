/**
 * Dev-only floating widget for cycling through stress states. Only renders
 * when `import.meta.env.DEV` is true; the call site is gated, so this entire
 * file tree-shakes out of production builds.
 *
 * Click a button to override the situational snapshot with that score so
 * the orb redraws in the corresponding colour tier without waiting for real
 * signals or restarting the dev server.
 */

interface Props {
  current: number | null;
  onSet: (score: number | null) => void;
}

const TIERS: { score: number; label: string }[] = [
  { score: 95, label: "Calm" },
  { score: 70, label: "Normal" },
  { score: 50, label: "Dense" },
  { score: 30, label: "Heavy" },
  { score: 10, label: "Overload" },
];

export default function DevControls({ current, onSet }: Props) {
  return (
    <div className="dev-controls">
      <div className="dev-controls-label">DEV · Stress</div>
      <div className="dev-controls-row">
        {TIERS.map((t) => (
          <button
            key={t.score}
            className={`dev-controls-btn ${current === t.score ? "is-current" : ""}`}
            onClick={() => onSet(t.score)}
            title={`${t.label} (${t.score})`}
          >
            {t.score}
          </button>
        ))}
        <button
          className={`dev-controls-btn ${current === null ? "is-current" : ""}`}
          onClick={() => onSet(null)}
          title="Use real snapshot"
        >
          Real
        </button>
      </div>
    </div>
  );
}
