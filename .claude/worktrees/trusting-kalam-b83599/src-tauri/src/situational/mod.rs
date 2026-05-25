mod collectors;
mod score;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use tauri::State;

pub use collectors::start_all;
pub use score::DayLabel;

pub struct SituationalState {
    // Continuous-time signals
    pub last_break_at: Instant,
    pub continuous_screen_min: u64,

    // Meeting signals (populated in step 3)
    pub in_meeting: bool,
    pub current_meeting_min: u64,
    pub cumulative_meeting_min_today: u64,
    pub back_to_back_count: u32,
    pub last_meeting_ended_at: Option<Instant>,

    // App / context-switching (populated in step 4)
    pub current_app_bundle: Option<String>,
    pub app_switches_last_30min: u32,
    pub focus_block_min: u64,

    // Input intensity (populated in step 5)
    pub keystrokes_per_min: u32,
    pub clicks_per_min: u32,

    // System context
    pub after_hours: bool,

    // Computed outputs (refreshed by score_loop every 60s)
    pub niyora_index: u8,
    pub current_interval_min: u64,
    pub day_label: DayLabel,
    pub contextual_message: String,

    // Internal: last day we reset cumulative counters
    pub last_reset_date: chrono::NaiveDate,
}

impl SituationalState {
    pub fn new() -> Self {
        Self {
            last_break_at: Instant::now(),
            continuous_screen_min: 0,
            in_meeting: false,
            current_meeting_min: 0,
            cumulative_meeting_min_today: 0,
            back_to_back_count: 0,
            last_meeting_ended_at: None,
            current_app_bundle: None,
            app_switches_last_30min: 0,
            focus_block_min: 0,
            keystrokes_per_min: 0,
            clicks_per_min: 0,
            after_hours: false,
            niyora_index: 100,
            current_interval_min: 90,
            day_label: DayLabel::Calm,
            contextual_message: "A clean start. Settle into the breath.".to_string(),
            last_reset_date: chrono::Local::now().date_naive(),
        }
    }
}

pub struct SituationalStateHandle(pub Arc<Mutex<SituationalState>>);

#[derive(Serialize)]
pub struct SituationalSnapshot {
    pub score: u8,
    pub interval_min: u64,
    pub label: String,
    pub contextual_message: String,
    pub in_meeting: bool,
    pub current_app: Option<String>,
    pub continuous_screen_min: u64,
    pub cumulative_meeting_min_today: u64,
    pub after_hours: bool,
}

#[tauri::command]
pub fn get_situational_snapshot(
    state: State<'_, SituationalStateHandle>,
) -> SituationalSnapshot {
    let s = state.0.lock().unwrap();
    SituationalSnapshot {
        score: s.niyora_index,
        interval_min: s.current_interval_min,
        label: s.day_label.as_str().to_string(),
        contextual_message: s.contextual_message.clone(),
        in_meeting: s.in_meeting,
        current_app: s.current_app_bundle.clone(),
        continuous_screen_min: s.continuous_screen_min,
        cumulative_meeting_min_today: s.cumulative_meeting_min_today,
        after_hours: s.after_hours,
    }
}
