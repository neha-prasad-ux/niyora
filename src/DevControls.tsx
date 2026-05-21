/**
 * Dev-only floating widget for cycling through stress states and
 * previewing screens that need real backend data. Only renders when
 * `import.meta.env.DEV` is true; the call site is gated, so this entire
 * file tree-shakes out of production builds.
 */

interface Props {
  current: number | null;
  onSet: (score: number | null) => void;
  /** Inject synthetic pre+post HRV for the current session and jump to
   *  the mood/reveal view so the post-session reveal can be inspected
   *  without going through the iPhone PPG flow. */
  onPreviewReveal?: () => void;
}

const TIERS: { score: number; label: string }[] = [
  { score: 95, label: "Calm" },
  { score: 70, label: "Normal" },
  { score: 50, label: "Dense" },
  { score: 30, label: "Heavy" },
  { score: 10, label: "Overload" },
];

export default function DevControls({ current, onSet, onPreviewReveal }: Props) {
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
      {onPreviewReveal && (
        <>
          <div className="dev-controls-label">DEV · Reveal</div>
          <div className="dev-controls-row">
            <button
              className="dev-controls-btn"
              onClick={onPreviewReveal}
              title="Inject synthetic pre+post HRV and jump to the reveal screen"
            >
              Preview
            </button>
          </div>
        </>
      )}
    </div>
  );
}
