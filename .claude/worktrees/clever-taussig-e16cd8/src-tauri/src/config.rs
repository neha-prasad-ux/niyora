use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Application data directory: `~/Library/Application Support/com.niyora.breathing/`
/// on macOS. Used for config, analytics events, and session history.
/// Created on first access.
pub fn app_data_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("com.niyora.breathing");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

#[derive(Default, Serialize, Deserialize)]
pub struct NiyoraConfig {
    /// Date (YYYY-MM-DD, local) of last PSS-4 prompt completion or dismissal.
    /// Used to gate the weekly Sunday check-in to one prompt per Sunday.
    #[serde(default)]
    pub last_pss4_date: Option<String>,
    /// True once the user has finished the first-launch onboarding flow.
    /// Determines whether App.tsx shows the onboarding view or the main panel.
    #[serde(default)]
    pub onboarded: bool,
    /// Anonymous analytics consent. `None` = not asked yet, `Some(true)` =
    /// opted in, `Some(false)` = declined. Set on the onboarding consent slide.
    #[serde(default)]
    pub analytics_consent: Option<bool>,
    /// Random anonymous identifier for PostHog. Generated once, only when the
    /// user opts in. Never tied to any personal information.
    #[serde(default)]
    pub analytics_id: Option<String>,
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
