import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onDone: () => void;
}

interface Slide {
  eyebrow?: string;
  title: string;
  body?: string;
  bullets?: string[];
  /** Optional override for how long this slide auto-displays before
   * advancing. Defaults to SLIDE_DURATION_MS. Bullet-heavy slides need
   * more time so the user can actually read them. */
  durationMs?: number;
}

const SLIDES: Slide[] = [
  {
    eyebrow: "Niyora",
    title: "Calm in 60 seconds.",
    body: "For founders, sales, devs, PMs, designers.",
    bullets: [
      "Data stays on your Mac",
      "We notice long screen time",
      "Soft reminder when you need it",
      "One minute. Back to work.",
    ],
    // Bullets stagger in over ~1.4s, then user needs time to read them.
    durationMs: 7000,
  },
  {
    eyebrow: "Made by",
    title: "Neha Prasad",
    body: "A doc (in Indian system of medicine) turned designer.",
  },
];

const SLIDE_DURATION_MS = 3800;

export default function Onboarding({ onDone }: Props) {
  const [index, setIndex] = useState(0);
  const [finishing, setFinishing] = useState(false);

  // Auto-advance through slides 0 → 1 → 2, then stop on the final slide
  // so the user can read it and click Begin at their own pace.
  useEffect(() => {
    if (index >= SLIDES.length - 1) return;
    const ms = SLIDES[index].durationMs ?? SLIDE_DURATION_MS;
    const t = setTimeout(() => setIndex((i) => i + 1), ms);
    return () => clearTimeout(t);
  }, [index]);

  const handleBegin = async () => {
    if (finishing) return;
    setFinishing(true);
    // Trigger the macOS notification permission popup. Fire-and-forget.
    try {
      await invoke("request_notification_permission");
    } catch {
      /* ignore — user can still use the app without notifications */
    }
    try {
      await invoke("mark_onboarded");
    } catch {
      /* ignore — worst case onboarding shows again */
    }
    onDone();
  };

  const slide = SLIDES[index];
  const isLast = index === SLIDES.length - 1;

  // Click anywhere on the screen advances to the next slide.
  // On the last slide, the click triggers Begin (mark onboarded + permission).
  const handleScreenClick = () => {
    if (finishing) return;
    if (isLast) {
      handleBegin();
    } else {
      setIndex((i) => i + 1);
    }
  };

  return (
    <div className="niyora-onboarding" onClick={handleScreenClick} style={{ cursor: "pointer" }}>
      <div className="onboarding-backdrop" />
      <div className="onboarding-content">
        {/* Top: dots showing progress */}
        <div className="onboarding-dots">
          {SLIDES.map((_, i) => (
            <button
              key={i}
              className={`onboarding-dot ${i === index ? "is-current" : ""} ${i < index ? "is-passed" : ""}`}
              onClick={(e) => { e.stopPropagation(); setIndex(i); }}
              aria-label={`Slide ${i + 1}`}
            />
          ))}
        </div>

        {/* Middle: the slide content. Keyed on index so React re-mounts
            and the fade-in animation re-plays each transition. */}
        <div key={index} className="onboarding-slide">
          <img
            src="/icons/niyora-logo.png"
            alt="Niyora"
            className="onboarding-logo"
            onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
          />
          {slide.eyebrow && (
            <div className="onboarding-eyebrow">{slide.eyebrow}</div>
          )}
          <div className="onboarding-title">{slide.title}</div>
          {slide.body && <div className="onboarding-body">{slide.body}</div>}
          {slide.bullets && (
            <ul className="onboarding-checklist">
              {slide.bullets.map((b, i) => (
                <li
                  key={i}
                  className="onboarding-bullet"
                  style={{ animationDelay: `${0.4 + i * 0.25}s` }}
                >
                  <span className="onboarding-check">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  </span>
                  <span>{b}</span>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Bottom: Begin button — only enabled on the last slide */}
        <button
          className="onboarding-btn"
          onClick={(e) => { e.stopPropagation(); handleBegin(); }}
          disabled={finishing}
        >
          {finishing ? "Welcome…" : "Begin"}
        </button>
      </div>
    </div>
  );
}
