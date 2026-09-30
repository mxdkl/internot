//! `CommIntensity` + `comm_intensity()`.
//!
//! Given a tie's procedural strength at time t, plus a compact
//! `PersonalityProjection` for each endpoint, return per-mode
//! communication rates. Services consume these to drive the
//! event-enumeration in `graph::events`.
//!
//! Rate composition:
//! ```text
//!   λ_mode = base_rate_mode
//!          * strength_factor(tie.strength)
//!          * personality_factor(self, peer, mode)
//!          * diurnal_factor(t_of_day)
//!          * weekly_factor(weekday)
//! ```
//!
//! v1 uses a coarse model: base rates from
//! `docs/superpowers/research/2026-05-14-communication-patterns.md`
//! (mail ~0.5/day for strong ties; chat ~5/day for cohabiting;
//! calendar ~0.05/day). Personality factors are simple linear
//! modifiers on Big-Five extraversion + conscientiousness.

use chrono::{DateTime, Datelike, Timelike, Utc};

/// Compact projection of a person's communication-relevant personality
/// dimensions. Consumers (e.g. `internot::social`) build this from
/// their full personality data.
#[derive(Debug, Clone, Copy)]
pub struct PersonalityProjection {
    /// Big Five extraversion in `[0, 1]`. Higher → more comms.
    pub extraversion: f64,
    /// Big Five conscientiousness in `[0, 1]`. Higher → more
    /// scheduled / formal comms.
    pub conscientiousness: f64,
    /// Chronotype: `0.0` = early bird, `1.0` = night owl.
    pub chronotype: f64,
}

impl Default for PersonalityProjection {
    fn default() -> Self {
        Self {
            extraversion: 0.5,
            conscientiousness: 0.5,
            chronotype: 0.5,
        }
    }
}

/// Per-mode communication rates between a tie pair at time `t`.
/// All in events-per-day units.
#[derive(Debug, Clone, Copy)]
pub struct CommIntensity {
    pub mail_per_day: f64,
    pub chat_per_day: f64,
    pub calendar_per_day: f64,
}

/// Compute `CommIntensity` for the tie at `t`. `strength` is the
/// output of [`crate::graph::tie_strength`].
pub fn comm_intensity(
    strength: f64,
    self_p: &PersonalityProjection,
    peer_p: &PersonalityProjection,
    t: DateTime<Utc>,
) -> CommIntensity {
    // Base event rates (events/day) from `2026-05-14-communication-patterns.md`.
    const BASE_MAIL_PER_DAY: f64 = 0.5;
    const BASE_CHAT_PER_DAY: f64 = 5.0;
    const BASE_CALENDAR_PER_DAY: f64 = 0.05;
    // Diurnal phase parameters: 24h cosine centered at peak_hour;
    // peak_hour is in [12, 16] depending on average chronotype.
    const DIURNAL_PEAK_BASE: f64 = 12.0;
    const DIURNAL_PEAK_SHIFT: f64 = 4.0;
    // Weekend multipliers: weekday baseline 1.0; weekend boosts
    // chat (more social use), suppresses mail/calendar (less work).
    const MAIL_WEEKEND_FACTOR: f64 = 0.7;
    const CHAT_WEEKEND_FACTOR: f64 = 1.2;
    const CAL_WEEKEND_FACTOR: f64 = 0.4;

    // Strength factor: linear in strength.
    let sf = strength.clamp(0.0, 1.0);
    // Personality factor: mean of extraversion + conscientiousness across endpoints.
    let ext = 0.5 * (self_p.extraversion + peer_p.extraversion);
    let cons = 0.5 * (self_p.conscientiousness + peer_p.conscientiousness);
    // 0.5 and 1.5 below are the slope/intercept of the linear blend — not base rates.
    let person_chat = 0.5 + ext;          // 0.5x at ext=0, 1.5x at ext=1
    let person_mail = 0.5 + 0.5 * ext + 0.5 * cons;
    let person_cal = 0.5 + cons;
    // Diurnal: peak shifts with chronotype (early-bird → 12:00,
    // night-owl → 16:00, default 0.5 → 14:00). 24-hour cosine; trough
    // is 12 hours from peak.
    let avg_chronotype = 0.5 * (self_p.chronotype + peer_p.chronotype);
    let peak_hour = DIURNAL_PEAK_BASE + DIURNAL_PEAK_SHIFT * avg_chronotype;
    let hour = t.hour() as f64;
    let diurnal = (0.5 + 0.5 * crate::dmath::cos((hour - peak_hour) * std::f64::consts::PI / 12.0)).clamp(0.0, 1.0);
    // Weekly: weekdays slightly higher than weekends for mail/calendar;
    // chat slightly the other way.
    let weekday = t.weekday().num_days_from_monday(); // 0 (Mon) .. 6 (Sun)
    let is_weekend = weekday >= 5;
    let mail_wf = if is_weekend { MAIL_WEEKEND_FACTOR } else { 1.0 };
    let chat_wf = if is_weekend { CHAT_WEEKEND_FACTOR } else { 1.0 };
    let cal_wf = if is_weekend { CAL_WEEKEND_FACTOR } else { 1.0 };
    CommIntensity {
        mail_per_day: BASE_MAIL_PER_DAY * sf * person_mail * diurnal * mail_wf,
        chat_per_day: BASE_CHAT_PER_DAY * sf * person_chat * diurnal * chat_wf,
        calendar_per_day: BASE_CALENDAR_PER_DAY * sf * person_cal * diurnal * cal_wf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap()
    }

    #[test]
    fn intensity_is_zero_when_strength_zero() {
        let p = PersonalityProjection::default();
        let i = comm_intensity(0.0, &p, &p, at(2025, 6, 1, 14));
        assert_eq!(i.mail_per_day, 0.0);
        assert_eq!(i.chat_per_day, 0.0);
        assert_eq!(i.calendar_per_day, 0.0);
    }

    #[test]
    fn intensity_is_higher_for_extraverts() {
        let intro = PersonalityProjection { extraversion: 0.1, ..Default::default() };
        let extra = PersonalityProjection { extraversion: 0.9, ..Default::default() };
        let i_intro = comm_intensity(0.9, &intro, &intro, at(2025, 6, 1, 14));
        let i_extra = comm_intensity(0.9, &extra, &extra, at(2025, 6, 1, 14));
        assert!(i_extra.chat_per_day > i_intro.chat_per_day);
        assert!(i_extra.mail_per_day > i_intro.mail_per_day);
    }

    #[test]
    fn intensity_drops_at_night() {
        let p = PersonalityProjection::default();
        let day = comm_intensity(0.9, &p, &p, at(2025, 6, 1, 14));
        let night = comm_intensity(0.9, &p, &p, at(2025, 6, 1, 3));
        assert!(night.chat_per_day < day.chat_per_day);
    }

    #[test]
    fn weekend_calendar_rate_is_lower() {
        let p = PersonalityProjection::default();
        let mon = comm_intensity(0.9, &p, &p, at(2025, 6, 2, 14)); // Monday
        let sat = comm_intensity(0.9, &p, &p, at(2025, 6, 7, 14)); // Saturday
        assert!(sat.calendar_per_day < mon.calendar_per_day);
    }

    #[test]
    fn calendar_rate_is_higher_for_conscientious() {
        let lazy = PersonalityProjection { conscientiousness: 0.1, ..Default::default() };
        let conscientious = PersonalityProjection { conscientiousness: 0.9, ..Default::default() };
        let i_lazy = comm_intensity(0.9, &lazy, &lazy, at(2025, 6, 2, 14)); // weekday noon-ish
        let i_cons = comm_intensity(0.9, &conscientious, &conscientious, at(2025, 6, 2, 14));
        assert!(
            i_cons.calendar_per_day > i_lazy.calendar_per_day,
            "expected conscientious calendar rate > lazy; got cons={:.4} vs lazy={:.4}",
            i_cons.calendar_per_day,
            i_lazy.calendar_per_day,
        );
    }

    #[test]
    fn chronotype_shifts_diurnal_peak() {
        // At 10:00 (morning), an early-bird pair should have higher
        // chat rate than a night-owl pair (peak shifts from 12 to 16).
        let early = PersonalityProjection { chronotype: 0.0, ..Default::default() };
        let late = PersonalityProjection { chronotype: 1.0, ..Default::default() };
        let morning = at(2025, 6, 2, 10);
        let i_early = comm_intensity(0.9, &early, &early, morning);
        let i_late = comm_intensity(0.9, &late, &late, morning);
        assert!(
            i_early.chat_per_day > i_late.chat_per_day,
            "at 10:00 early-bird ({:.4}) should chat more than night-owl ({:.4})",
            i_early.chat_per_day,
            i_late.chat_per_day,
        );
        // Symmetric: at 18:00 (evening), night-owl should chat more.
        let evening = at(2025, 6, 2, 18);
        let i_early_e = comm_intensity(0.9, &early, &early, evening);
        let i_late_e = comm_intensity(0.9, &late, &late, evening);
        assert!(
            i_late_e.chat_per_day > i_early_e.chat_per_day,
            "at 18:00 night-owl ({:.4}) should chat more than early-bird ({:.4})",
            i_late_e.chat_per_day,
            i_early_e.chat_per_day,
        );
    }
}
