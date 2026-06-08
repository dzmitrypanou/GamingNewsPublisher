use chrono::{DateTime, Local, Utc};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
struct Schedule {
    next_fetch_at: Option<DateTime<Local>>,
    scheduled_delay_secs: u64,
}

pub struct FetchSchedulerRuntime {
    inner: Mutex<Schedule>,
}

impl FetchSchedulerRuntime {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Schedule::default()),
        }
    }

    pub fn set_next(&self, at: DateTime<Local>, delay: Duration) {
        let secs = delay.as_secs().max(1);
        let mut guard = self.inner.lock().unwrap();
        guard.scheduled_delay_secs = secs;
        guard.next_fetch_at = Some(at);
    }

    pub fn clear(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.next_fetch_at = None;
        guard.scheduled_delay_secs = 0;
    }

    pub fn next_fetch_at(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .next_fetch_at
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
    }

    pub fn scheduled_delay_secs(&self) -> u64 {
        self.inner.lock().unwrap().scheduled_delay_secs
    }
}
