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
    // Productive zone: single-app focus up to 90 min with few context switches.
    // Skipped during after-hours so the late+focus compound rule isn't partially cancelled.
    if s.focus_block_min > 45 && s.focus_block_min <= 90 && s.app_switches_last_30min < 5
        && !s.after_hours
    {
        t += 15;
    }
    // 90+ min in the same app without a break is itself an overwork signal.
    if s.focus_block_min > 90 { t -= 20; }
    // Past 2 hours of unbroken focus: additional urgency.
    if s.focus_block_min > 120 { t -= 10; }
    // Working late while still in a sustained focus block: compound signal.
    if s.after_hours && s.focus_block_min > 60 { t -= 10; }
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

#[cfg(test)]
mod tests {
    use super::compute_interval;
    use crate::situational::SituationalState;

    // Tests assert relative ordering rather than exact values so they survive
    // future tuning of the base constant.

    #[test]
    fn focus_bonus_in_productive_zone() {
        let mut s = SituationalState::new();
        s.focus_block_min = 60;
        s.app_switches_last_30min = 2;
        s.cumulative_meeting_min_today = 1; // suppress calm-day bonus so focus bonus is the only upward signal
        assert!(
            compute_interval(&s) > 90,
            "productive focus block should lengthen interval above baseline"
        );
    }

    #[test]
    fn focus_block_above_90_shortens_interval() {
        // switches = 5 suppresses the focus bonus for both states so only the
        // overwork penalty differs across the 90-min boundary.
        // meetings = 1 suppresses the calm-day bonus for both.
        let mut s_long = SituationalState::new();
        s_long.focus_block_min = 91;
        s_long.app_switches_last_30min = 5;
        s_long.cumulative_meeting_min_today = 1;

        let mut s_short = SituationalState::new();
        s_short.focus_block_min = 89;
        s_short.app_switches_last_30min = 5;
        s_short.cumulative_meeting_min_today = 1;

        assert!(
            compute_interval(&s_long) < compute_interval(&s_short),
            "91-min focus block should fire sooner than 89-min block"
        );
    }

    #[test]
    fn focus_block_above_120_stacks_further() {
        let mut s_130 = SituationalState::new();
        s_130.focus_block_min = 130;

        let mut s_100 = SituationalState::new();
        s_100.focus_block_min = 100;

        assert!(
            compute_interval(&s_130) < compute_interval(&s_100),
            "130-min focus block should fire sooner than 100-min block"
        );
    }

    #[test]
    fn after_hours_plus_long_focus_compounds() {
        // Late + sustained focus fires sooner than late alone.
        let mut s_compound = SituationalState::new();
        s_compound.after_hours = true;
        s_compound.focus_block_min = 70;

        let mut s_base = SituationalState::new();
        s_base.after_hours = true;
        s_base.focus_block_min = 10;

        assert!(
            compute_interval(&s_compound) < compute_interval(&s_base),
            "after_hours + 70-min focus should fire sooner than after_hours alone"
        );
    }
}
