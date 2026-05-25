use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn default_true() -> bool { true }

/// Application data directory: `~/Library/Application Support/com.niyora.breathing/`
/// on macOS. Used for config, analytics events, and session history.
/// Created on first access.
pub fn app_data_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("com.niyora.breathing");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

#[derive(Serialize, Deserialize)]
pub struct NiyoraConfig {
    /// Date (YYYY-MM-DD, local) of last PSS-4 prompt completion or dismissal.
    /// Used to gate the weekly Sunday check-in to one prompt per Sunday.
    #[serde(default)]
    pub last_pss4_date: Option<String>,
    /// True once the user has finished the first-launch onboarding flow.
    /// Determines whether App.tsx shows the onboarding view or the main panel.
    #[serde(default)]
    pub onboarded: bool,
    /// Local YYYY-MM-DD of the most recent panel open. Stamped by
    /// `claim_first_open_today`; used to swap the intro copy on the first
    /// open of each day ("Start your day with a breath.").
    #[serde(default)]
    pub last_opened_date: Option<String>,
    /// Stable identity of this Mac as seen by paired iPhones. Minted on
    /// first launch of the companion service; persisted so a phone's
    /// stored secret keeps mapping to the same server across reboots.
    #[serde(default)]
    pub companion_server_id: Option<String>,
    /// Whether to register as a macOS Login Item so the tray icon
    /// reappears after a reboot. Default `true` for new installs.
    /// Persisted so the setting survives app updates.
    #[serde(default = "default_true")]
    pub launch_at_login: bool,
    /// Anonymous analytics consent. `None` = not asked yet, `Some(true)` =
    /// opted in, `Some(false)` = declined. Set on the onboarding consent slide.
    #[serde(default)]
    pub analytics_consent: Option<bool>,
    /// Random anonymous identifier for PostHog. Generated once, only when the
    /// user opts in. Never tied to any personal information.
    #[serde(default)]
    pub analytics_id: Option<String>,
    /// Preferred ambient track for breathing sessions: "serene", "ocean",
    /// "forest", or "random". None treated as "random".
    #[serde(default)]
    pub preferred_track: Option<String>,
}

impl Default for NiyoraConfig {
    fn default() -> Self {
        Self {
            last_pss4_date: None,
            onboarded: false,
            last_opened_date: None,
            companion_server_id: None,
            launch_at_login: true,
            analytics_consent: None,
            analytics_id: None,
            preferred_track: None,
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    Some(app_data_dir()?.join("niyora_config.json"))
}

pub fn load() -> NiyoraConfig {
    let Some(path) = config_path() else { return NiyoraConfig::default(); };
    let Ok(contents) = std::fs::read_to_string(&path) else { return NiyoraConfig::default(); };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save(cfg: &NiyoraConfig) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "Could not resolve config directory".to_string())?;
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_preferred_track() -> String {
    load().preferred_track.unwrap_or_else(|| "random".to_string())
}

/// Allowed values for the preferred track. Anything else is rejected so the
/// config file does not become a junk drawer of arbitrary IPC strings.
/// Kept in sync with `AMBIENT_TRACKS` / `resolveTrack` in BreathingSession.tsx.
const ALLOWED_TRACKS: &[&str] = &["random", "serene", "ocean", "forest"];

#[tauri::command]
pub fn set_preferred_track(track: String) -> Result<(), String> {
    if !ALLOWED_TRACKS.contains(&track.as_str()) {
        return Err(format!("unknown track: {track}"));
    }
    let mut cfg = load();
    cfg.preferred_track = Some(track);
    save(&cfg)
}
