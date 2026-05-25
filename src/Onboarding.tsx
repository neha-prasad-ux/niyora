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
  /** "consent" renders the analytics opt-in slide: a detail panel plus two
   * explicit buttons instead of click-anywhere-to-advance. */
  kind?: "consent";
  /** Bold heading line at the top of the note panel. */
  noteHead?: string;
  /** Secondary detail text, rendered in a panel below the body. */
  note?: string;
}

const SLIDES: Slide[] = [
  {
    eyebrow: "Niyora",
    title: "Calm in 60 seconds.",
    body: "For founders, sales, devs, PMs, designers.",
    bullets: [
      "Data stays on your Mac",
      "We spot stress in your work patterns",
      "We notify you when you need calm",
      "Breathing and mindful practices, combined",
      "Every practice under 60 seconds",
    ],
  },
  {
    kind: "consent",
    title: "Private by design",
    noteHead: "Built independently, shaped by you.",
    note: "Anonymous usage tells us what to make better next.\n\nStress scores, breath patterns, anything that identifies you. None of it leaves your Mac.\n\nNiyora launches with your Mac so it's there when you need it.",
  },
];

const SLIDE_DURATION_MS = 3800;

export default function Onboarding({ onDone }: Props) {
  const [index, setIndex] = useState(0);
  const [finishing, setFinishing] = useState(false);

  // The first slide (value-prop with the bullet list) waits for a click .
  // it has the most to read and shouldn't disappear on people. Subsequent
  // slides auto-advance gently. The last slide always stops so the user
  // can answer the consent prompt at their own pace.
  useEffect(() => {
    if (index === 0) return;
    if (index >= SLIDES.length - 1) return;
    const ms = SLIDES[index].durationMs ?? SLIDE_DURATION_MS;
    const t = setTimeout(() => setIndex((i) => i + 1), ms);
    return () => clearTimeout(t);
  }, [index]);

  // Finish onboarding: record the analytics choice, trigger the macOS
  // notification permission popup, mark onboarded, then hand off to the app.
  const finish = async (analyticsConsent: boolean) => {
    if (finishing) return;
    setFinishing(true);
    try {
      await invoke("set_analytics_consent", { granted: analyticsConsent });
    } catch {
      /* ignore. defaults to no consent, app still works */
    }
    // Trigger the macOS notification permission popup. Fire-and-forget.
    try {
      await invoke("request_notification_permission");
    } catch {
      /* ignore. user can still use the app without notifications */
    }
    try {
      await invoke("mark_onboarded");
    } catch {
      /* ignore. worst case onboarding shows again */
    }
    onDone();
  };

  const slide = SLIDES[index];
  const isConsent = slide.kind === "consent";

  // Click anywhere advances to the next slide. The consent slide is the
  // exception: it waits for an explicit button choice and ignores background
  // clicks so the user cannot skip the decision by tapping.
  const handleScreenClick = () => {
    if (finishing || isConsent) return;
    setIndex((i) => i + 1);
  };

  return (
    <div className="niyora-onboarding" onClick={handleScreenClick} style={{ cursor: isConsent ? "default" : "pointer" }}>
      <div className="onboarding-backdrop" />
      {/* "I live here" pointer . shown only on the first slide so users can
          see where to find Niyora later, without distracting once they're in. */}
      {index === 0 && (
        <div className="onboarding-here" aria-hidden="true">
          <svg className="onboarding-here-arrow" width="16" height="20" viewBox="0 0 16 20" fill="none">
            <path d="M8 2 L8 18 M8 2 L3 7 M8 2 L13 7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <span className="onboarding-here-text">I live up here.</span>
        </div>
      )}
      <div className="onboarding-content">
        {/* Progress dots only shown when there's more than one slide. */}
        {SLIDES.length > 1 && (
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
        )}

        {/* Middle: the slide content. Keyed on index so React re-mounts
            and the fade-in animation re-plays each transition. */}
        <div key={index} className="onboarding-slide">
          {isConsent ? (
            <svg className="onboarding-lock-icon" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <rect x="4" y="11" width="16" height="10" rx="2" />
              <path d="M8 11 V7 a4 4 0 0 1 8 0 V11" />
            </svg>
          ) : (
            <img
              src="/icons/niyora-logo.png"
              alt="Niyora"
              className="onboarding-logo"
              onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
            />
          )}
          {slide.eyebrow && (
            <div className="onboarding-eyebrow">{slide.eyebrow}</div>
          )}
          <div className="onboarding-title">{slide.title}</div>
          {slide.body && <div className="onboarding-body">{slide.body}</div>}
          {(slide.note || slide.noteHead) && (
            <div className="onboarding-note">
              {slide.noteHead && <div className="onboarding-note-head">{slide.noteHead}</div>}
              {slide.note}
            </div>
          )}
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

        {/* Bottom: consent slide gets two explicit buttons, every other
            slide gets a single Continue button. */}
        {isConsent ? (
          <div className="onboarding-consent-actions">
            <button
              className="onboarding-btn"
              onClick={(e) => { e.stopPropagation(); finish(true); }}
              disabled={finishing}
            >
              {finishing ? "Welcome…" : "Share anonymous analytics"}
            </button>
            <button
              className="onboarding-btn onboarding-btn-secondary"
              onClick={(e) => { e.stopPropagation(); finish(false); }}
              disabled={finishing}
            >
              Continue without analytics
            </button>
          </div>
        ) : (
          <button
            className="onboarding-btn"
            onClick={(e) => { e.stopPropagation(); setIndex((i) => i + 1); }}
            disabled={finishing}
          >
            Continue
          </button>
        )}
      </div>
    </div>
  );
}
