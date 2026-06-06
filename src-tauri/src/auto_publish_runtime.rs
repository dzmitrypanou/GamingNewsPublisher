use chrono::{DateTime, Utc};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
struct Schedule {
    next_publish_at: Option<DateTime<Utc>>,
    scheduled_delay_secs: u64,
}

pub struct AutoPublishRuntime {
    inner: Mutex<Schedule>,
}

impl AutoPublishRuntime {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Schedule::default()),
        }
    }

    pub fn set_next(&self, delay: Duration) {
        let secs = delay.as_secs().max(1);
        let mut guard = self.inner.lock().unwrap();
        guard.scheduled_delay_secs = secs;
        guard.next_publish_at = Some(Utc::now() + chrono::Duration::seconds(secs as i64));
    }

    pub fn clear(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.next_publish_at = None;
        guard.scheduled_delay_secs = 0;
    }

    pub fn next_publish_at(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .next_publish_at
            .map(|dt| dt.to_rfc3339())
    }

    pub fn scheduled_delay_secs(&self) -> u64 {
        self.inner.lock().unwrap().scheduled_delay_secs
    }
}
