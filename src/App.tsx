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
    listen("onboarding_reset", () => {
      setNeedsOnboarding(true);
    })
      .then((u) => {
        unlisten = u;
      })
      .catch(() => {
        /* event plugin unavailable in dev preview */
      });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Dev-only listener: when the tray fires "Reset sessions", bump openKey
  // so useSessionStats refetches and the next session is treated as the
  // first (Box Breath default + AHA screen).
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen("sessions_reset", () => {
      setView("main");
      setOpenKey((k) => k + 1);
    })
      .then((u) => {
        unlisten = u;
      })
      .catch(() => {
        /* event plugin unavailable in dev preview */
      });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);
  // Bumped each time the panel becomes visible. Used as a `key` on
  // BreathingSession so React tears down + remounts it with fresh state
  // (new random technique, intro reset, etc.) on every tray click — without
  // reloading the whole page.
  const [openKey, setOpenKey] = useState(0);
  // Dev-only stress override. When non-null, replaces the real snapshot
  // so we can preview every tier without waiting for real signals.
  const [devStress, setDevStress] = useState<number | null>(null);
  const realSnapshot = useSnapshot();
  const snapshot = devStress !== null ? synthesizeSnapshot(devStress) : realSnapshot;
  const stats = useSessionStats(openKey);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen("panel_did_show", () => {
      setView("main");
      setOpenKey((k) => k + 1);
    })
      .then((u) => {
        unlisten = u;
      })
      .catch(() => {
        /* event plugin unavailable in dev preview — no-op */
      });
    return () => {
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
    return <PostSessionMood onDone={handleMoodDone} />;
  }

  if (view === "pss4") {
    return <PssFour onDone={() => setView("settings")} />;
  }

  return (
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <BreathingSession
        key={openKey}
        onComplete={handleSessionComplete}
        snapshot={snapshot}
        completedSessions={stats.completed}
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
        <DevControls current={devStress} onSet={setDevStress} />
      )}
    </div>
  );
}

export default App;
