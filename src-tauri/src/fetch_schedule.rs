use crate::models::AppSettings;
use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone};
use std::time::Duration as StdDuration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchScheduleConfig {
    pub enabled: bool,
    pub start_at: String,
    pub repeat_unit: String,
    pub repeat_every: u32,
}

impl FetchScheduleConfig {
    pub fn from_settings(settings: &AppSettings) -> Self {
        let repeat_unit = if settings.fetch_repeat_unit.trim().is_empty() {
            "minutes".to_string()
        } else {
            settings.fetch_repeat_unit.clone()
        };
        let repeat_every = if settings.fetch_repeat_every > 0 {
            settings.fetch_repeat_every
        } else {
            settings.fetch_interval_minutes.max(1)
        };

        Self {
            enabled: settings.auto_fetch,
            start_at: settings.fetch_schedule_start_at.clone(),
            repeat_unit,
            repeat_every: repeat_every.max(1),
        }
    }

    pub fn interval_minutes(&self) -> u32 {
        match self.repeat_unit.as_str() {
            "hours" => self.repeat_every.saturating_mul(60),
            "days" => self.repeat_every.saturating_mul(24 * 60),
            _ => self.repeat_every,
        }
    }
}

pub fn parse_local_start_at(raw: &str) -> Option<DateTime<Local>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M"))
        .ok()
        .and_then(|naive| Local.from_local_datetime(&naive).single())
}

pub fn parse_last_fetch_at(raw: &str) -> Option<DateTime<Local>> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
}

pub fn compute_next_fetch(
    config: &FetchScheduleConfig,
    now: DateTime<Local>,
    last_fetch_at: Option<DateTime<Local>>,
) -> Option<(DateTime<Local>, StdDuration)> {
    if !config.enabled {
        return None;
    }

    let startup_delay = StdDuration::from_secs(5);

    if let Some(start) = parse_local_start_at(&config.start_at) {
        if now < start {
            let delay = (start - now)
                .to_std()
                .unwrap_or(startup_delay)
                .max(StdDuration::from_secs(1));
            return Some((start, delay));
        }

        if last_fetch_at.is_none() {
            return Some((now, startup_delay));
        }

        let next = advance_from_anchor(start, now, config);
        return delay_until(now, next);
    }

    if last_fetch_at.is_none() {
        return Some((now, startup_delay));
    }

    let last = last_fetch_at?;
    let next = advance_from_last(last, config);
    delay_until(now, next)
}

fn delay_until(now: DateTime<Local>, next: DateTime<Local>) -> Option<(DateTime<Local>, StdDuration)> {
    if next <= now {
        return Some((now, StdDuration::from_secs(5)));
    }
    let delay = (next - now)
        .to_std()
        .unwrap_or(StdDuration::from_secs(5))
        .max(StdDuration::from_secs(1));
    Some((next, delay))
}

fn advance_from_anchor(
    start: DateTime<Local>,
    now: DateTime<Local>,
    config: &FetchScheduleConfig,
) -> DateTime<Local> {
    let every = config.repeat_every.max(1) as i64;
    match config.repeat_unit.as_str() {
        "hours" => advance_linear(start, now, every * 3600),
        "days" => advance_daily(start, now, every),
        _ => advance_linear(start, now, every * 60),
    }
}

fn advance_from_last(last: DateTime<Local>, config: &FetchScheduleConfig) -> DateTime<Local> {
    let every = config.repeat_every.max(1) as i64;
    match config.repeat_unit.as_str() {
        "hours" => last + Duration::hours(every),
        "days" => {
            let target = last.date_naive() + Duration::days(every);
            let naive = target.and_time(last.time());
            Local
                .from_local_datetime(&naive)
                .single()
                .unwrap_or_else(|| last + Duration::days(every))
        }
        _ => last + Duration::minutes(every),
    }
}

fn advance_linear(start: DateTime<Local>, now: DateTime<Local>, period_secs: i64) -> DateTime<Local> {
    let period_secs = period_secs.max(1);
    let elapsed = (now - start).num_seconds().max(0);
    let periods = elapsed / period_secs + 1;
    start + Duration::seconds(periods * period_secs)
}

fn advance_daily(start: DateTime<Local>, now: DateTime<Local>, every_days: i64) -> DateTime<Local> {
    let every_days = every_days.max(1);
    let start_date = start.date_naive();
    let now_date = now.date_naive();
    let days = (now_date - start_date).num_days().max(0);
    let periods = days / every_days + 1;
    let target_date = start_date + Duration::days(periods * every_days);
    let naive = target_date.and_time(start.time());
    Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, m, d, hh, mm, 0)
            .single()
            .expect("valid local datetime")
    }

    #[test]
    fn waits_until_start_at_in_future() {
        let config = FetchScheduleConfig {
            enabled: true,
            start_at: "2026-06-10T09:00".to_string(),
            repeat_unit: "minutes".to_string(),
            repeat_every: 30,
        };
        let now = local(2026, 6, 8, 12, 0);
        let (next, delay) = compute_next_fetch(&config, now, None).unwrap();
        assert_eq!(next, local(2026, 6, 10, 9, 0));
        assert!(delay.as_secs() > 3600);
    }

    #[test]
    fn fetches_soon_after_start_when_no_previous_fetch() {
        let config = FetchScheduleConfig {
            enabled: true,
            start_at: "2026-06-08T09:00".to_string(),
            repeat_unit: "days".to_string(),
            repeat_every: 1,
        };
        let now = local(2026, 6, 8, 10, 0);
        let (_, delay) = compute_next_fetch(&config, now, None).unwrap();
        assert_eq!(delay.as_secs(), 5);
    }

    #[test]
    fn daily_schedule_uses_anchor_time() {
        let config = FetchScheduleConfig {
            enabled: true,
            start_at: "2026-06-08T09:00".to_string(),
            repeat_unit: "days".to_string(),
            repeat_every: 1,
        };
        let now = local(2026, 6, 8, 15, 0);
        let last = local(2026, 6, 8, 9, 5);
        let (next, _) = compute_next_fetch(&config, now, Some(last)).unwrap();
        assert_eq!(next, local(2026, 6, 9, 9, 0));
    }

    #[test]
    fn rolling_minutes_from_last_fetch_without_start() {
        let config = FetchScheduleConfig {
            enabled: true,
            start_at: String::new(),
            repeat_unit: "minutes".to_string(),
            repeat_every: 30,
        };
        let last = local(2026, 6, 8, 10, 0);
        let now = local(2026, 6, 8, 10, 10);
        let (next, _) = compute_next_fetch(&config, now, Some(last)).unwrap();
        assert_eq!(next, local(2026, 6, 8, 10, 30));
    }
}
