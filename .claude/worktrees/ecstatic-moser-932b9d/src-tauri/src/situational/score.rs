use super::SituationalState;

#[derive(Clone, Copy, Debug)]
pub enum DayLabel {
    Calm,
    Normal,
    Dense,
    Heavy,
}

impl DayLabel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DayLabel::Calm => "calm",
            DayLabel::Normal => "normal",
            DayLabel::Dense => "dense",
            DayLabel::Heavy => "heavy",
        }
    }
}

pub fn recompute(s: &mut SituationalState) {
    s.niyora_index = compute_index(s);
    s.current_interval_min = compute_interval(s);
    s.day_label = label_from_index(s.niyora_index);
    s.contextual_message = pick_message(s);
}

fn clip01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn compute_index(s: &SituationalState) -> u8 {
    let break_recency_secs = s.last_break_at.elapsed().as_secs() as f32;
    let load = 0.25 * clip01(s.continuous_screen_min as f32 / 120.0)
        + 0.20 * clip01(s.cumulative_meeting_min_today as f32 / 360.0)
        + 0.15 * clip01(s.back_to_back_count as f32 / 3.0)
        + 0.15 * clip01(s.app_switches_last_30min as f32 / 40.0)
        + 0.10 * clip01(s.keystrokes_per_min as f32 / 250.0)
        + 0.10 * if s.after_hours { 1.0 } else { 0.0 }
        + 0.05 * clip01(break_recency_secs / 7200.0);
    ((1.0 - load) * 100.0).round().clamp(0.0, 100.0) as u8
}

fn compute_interval(s: &SituationalState) -> u64 {
    let mut t: i64 = 90;
    if s.continuous_screen_min > 90 { t -= 20; }
    if s.continuous_screen_min > 120 { t -= 15; }
    if s.cumulative_meeting_min_today > 240 { t -= 15; }
    if s.back_to_back_count >= 3 { t -= 15; }
    if s.app_switches_last_30min > 30 { t -= 10; }
    if s.keystrokes_per_min > 200 { t -= 10; }
    if s.after_hours { t -= 10; }
    if s.focus_block_min > 45 && s.app_switches_last_30min < 5 { t += 15; }
    if s.cumulative_meeting_min_today == 0 && s.last_break_at.elapsed().as_secs() < 3600 {
        t += 15;
    }
    t.clamp(40, 120) as u64
}

fn label_from_index(score: u8) -> DayLabel {
    match score {
        80..=100 => DayLabel::Calm,
        60..=79 => DayLabel::Normal,
        40..=59 => DayLabel::Dense,
        _ => DayLabel::Heavy,
    }
}

#[derive(Clone, Copy)]
enum Factor {
    ScreenTime,
    Meetings,
    Switching,
    Keystrokes,
    AfterHours,
    None,
}

fn dominant_factor(s: &SituationalState) -> Factor {
    let contributors = [
        (Factor::ScreenTime, 0.25 * clip01(s.continuous_screen_min as f32 / 120.0)),
        (Factor::Meetings,   0.20 * clip01(s.cumulative_meeting_min_today as f32 / 360.0)),
        (Factor::Switching,  0.15 * clip01(s.app_switches_last_30min as f32 / 40.0)),
        (Factor::Keystrokes, 0.10 * clip01(s.keystrokes_per_min as f32 / 250.0)),
        (Factor::AfterHours, 0.10 * if s.after_hours { 1.0 } else { 0.0 }),
    ];
    let max = contributors
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    match max {
        Some(&(f, v)) if v > 0.05 => f,
        _ => Factor::None,
    }
}

fn pick_message(s: &SituationalState) -> String {
    match dominant_factor(s) {
        Factor::ScreenTime => "A long stretch at the screen. Your nervous system is asking to soften.",
        Factor::Meetings   => "Back-to-back meetings keep the body in alert mode. One breath resets the dial.",
        Factor::Switching  => "Lots of context shifts. Let's bring everything back to one point.",
        Factor::Keystrokes => "The pace is high. A pause now will help the next stretch.",
        Factor::AfterHours => "It's late. The day deserves a soft landing.",
        Factor::None       => "A clean start. Settle into the breath.",
    }
    .to_string()
}
