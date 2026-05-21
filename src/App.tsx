import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import BreathingSession from "./BreathingSession";
import Settings from "./Settings";
import PostSessionMood from "./PostSessionMood";
import PssFour from "./PssFour";
import Onboarding from "./Onboarding";
import FirstSessionAha from "./FirstSessionAha";
import { useSnapshot, synthesizeSnapshot } from "./useSnapshot";
import { useSessionStats } from "./useSessionStats";
import DevControls from "./DevControls";
import "./App.css";

type View = "main" | "settings" | "mood" | "pss4" | "onboarding" | "first_session";

function App() {
  const [view, setView] = useState<View>("main");
  // null = still checking, true = needs onboarding, false = onboarded
  const [needsOnboarding, setNeedsOnboarding] = useState<boolean | null>(null);

  useEffect(() => {
    invoke<boolean>("is_onboarded")
      .then((done) => setNeedsOnboarding(!done))
      .catch(() => {
        // If the backend isn't available (dev preview), don't block the UI.
        setNeedsOnboarding(false);
      });
  }, []);

  // Dev-only listener: when the tray menu fires "Reset onboarding", flip
  // the state so the next panel open shows the onboarding flow again.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listen("onboarding_reset", () => {
      setNeedsOnboarding(true);
    })
      .then((u) => {
        // StrictMode double-invokes effects in dev. If cleanup already ran
        // before listen() resolved, unlisten immediately so we don't leave
        // a duplicate listener registered.
        if (cancelled) u();
        else unlisten = u;
      })
      .catch(() => {
        /* event plugin unavailable in dev preview */
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  // Dev-only listener: when the tray fires "Reset sessions", bump openKey
  // so useSessionStats refetches and the next session is treated as the
  // first (Box Breath default + AHA screen).
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listen("sessions_reset", () => {
      setView("main");
      setOpenKey((k) => k + 1);
    })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch(() => {
        /* event plugin unavailable in dev preview */
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);
  // Bumped each time the panel becomes visible. Used as a `key` on
  // BreathingSession so React tears down + remounts it with fresh state
  // (new random technique, intro reset, etc.) on every tray click — without
  // reloading the whole page.
  const [openKey, setOpenKey] = useState(0);
  // Stable id for the session a breathing-view mount represents. Minted
  // once per openKey so the pre-session "Measure stress" tap, the
  // record_session call when rounds finish, and the post-mood "Measure
  // stress" tap all agree on which row the iPhone's PPG capture binds
  // to. Crypto-strong UUID via the browser API (no extra dep).
  const [sessionId, setSessionId] = useState<string>(() => crypto.randomUUID());
  useEffect(() => {
    setSessionId(crypto.randomUUID());
  }, [openKey]);
  // True for the very first panel open of each local day. Drives the
  // "Start your day with a breath." intro copy and forces Box Breath so the
  // promise of ~60s holds.
  const [isFirstOpenToday, setIsFirstOpenToday] = useState(false);
  // Dev-only stress override. When non-null, replaces the real snapshot
  // so we can preview every tier without waiting for real signals.
  const [devStress, setDevStress] = useState<number | null>(null);
  const realSnapshot = useSnapshot();
  const snapshot = devStress !== null ? synthesizeSnapshot(devStress) : realSnapshot;
  const stats = useSessionStats(openKey);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    listen("panel_did_show", async () => {
      // Resolve day-start BEFORE bumping openKey so BreathingSession sees
      // the correct prop on first mount (createState runs once). The
      // backend call is a local file read — fast enough that the panel
      // doesn't feel laggy.
      let firstToday = false;
      try {
        firstToday = await invoke<boolean>("claim_first_open_today");
      } catch {
        firstToday = false;
      }
      setIsFirstOpenToday(firstToday);
      setView("main");
      setOpenKey((k) => k + 1);
    })
      .then((u) => {
        // Critical: without this, StrictMode's double-invoke leaves two
        // live listeners. claim_first_open_today would then run twice per
        // open — the second call returns false and clobbers isFirstOpenToday.
        if (cancelled) u();
        else unlisten = u;
      })
      .catch(() => {
        /* event plugin unavailable in dev preview — no-op */
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const handleSessionComplete = useCallback(async () => {
    const wasFirst = stats.completed === 0;
    try {
      await invoke("log_event", { eventType: "session_completed", meta: {} });
    } catch {
      /* ignore */
    }
    setView(wasFirst ? "first_session" : "mood");
  }, [stats.completed]);

  const handleMoodDone = useCallback(async () => {
    await invoke("hide_panel");
  }, []);

  // While checking onboarding state, render nothing — avoids a flash.
  if (needsOnboarding === null) return null;

  if (needsOnboarding) {
    return <Onboarding onDone={() => setNeedsOnboarding(false)} />;
  }

  if (view === "settings") {
    return <Settings onBack={() => setView("main")} onOpenPss4={() => setView("pss4")} />;
  }

  if (view === "first_session") {
    return <FirstSessionAha onContinue={() => setView("mood")} />;
  }

  if (view === "mood") {
    return (
      <PostSessionMood
        onDone={handleMoodDone}
        sessionId={sessionId}
        onSeeSoul={() => setView("settings")}
      />
    );
  }

  if (view === "pss4") {
    return <PssFour onDone={() => setView("settings")} />;
  }

  return (
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <BreathingSession
        key={openKey}
        sessionId={sessionId}
        onComplete={handleSessionComplete}
        snapshot={snapshot}
        completedSessions={stats.completed}
        isFirstOpenToday={isFirstOpenToday}
      />
      <button
        className="niyora-gear-btn niyora-tip"
        onClick={() => setView("settings")}
        data-tooltip="My Soul"
        title="My Soul"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="8" r="4" />
          <path d="M4 21c0-4.5 3.5-8 8-8s8 3.5 8 8" />
        </svg>
      </button>
      {import.meta.env.DEV && (
        <DevControls
          current={devStress}
          onSet={setDevStress}
          onPreviewReveal={() => {
            // Synthesize a calm-rising session (RMSSD 42 → 58) and jump
            // to the mood view; the reveal effect there will detect the
            // injected row and swap to PostSessionReveal.
            invoke("companion_inject_synthetic_reveal", {
              sessionId,
              preRmssdMs: 42,
              postRmssdMs: 58,
            })
              .then(() => setView("mood"))
              .catch((e) => console.warn("[dev preview]", e));
          }}
        />
      )}
    </div>
  );
}

export default App;
