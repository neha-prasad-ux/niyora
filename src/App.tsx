import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import BreathingSession from "./BreathingSession";
import "./App.css";

function App() {
  const handleComplete = useCallback(async () => {
    await invoke("hide_panel");
  }, []);

  return <BreathingSession onComplete={handleComplete} />;
}

export default App;
