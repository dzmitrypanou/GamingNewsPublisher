use crate::fetch::FetchCounters;
use crate::models::FetchResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct FetchRuntime {
    is_fetching: AtomicBool,
    last_result: Mutex<Option<FetchResult>>,
    active_counters: Mutex<Option<Arc<FetchCounters>>>,
}

impl FetchRuntime {
    pub fn new() -> Self {
        Self {
            is_fetching: AtomicBool::new(false),
            last_result: Mutex::new(None),
            active_counters: Mutex::new(None),
        }
    }

    pub fn try_begin(&self) -> bool {
        self.is_fetching
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn set_active_counters(&self, counters: Arc<FetchCounters>) {
        *self.active_counters.lock().unwrap() = Some(counters);
    }

    pub fn clear_active_counters(&self) {
        *self.active_counters.lock().unwrap() = None;
    }

    pub fn finish(&self, result: FetchResult) {
        *self.last_result.lock().unwrap() = Some(result);
        self.clear_active_counters();
        self.is_fetching.store(false, Ordering::SeqCst);
    }

    pub fn is_fetching(&self) -> bool {
        self.is_fetching.load(Ordering::SeqCst)
    }

    pub fn last_result(&self) -> Option<FetchResult> {
        self.last_result.lock().unwrap().clone()
    }

    pub fn live_snapshot(&self) -> Option<FetchResult> {
        self.active_counters
            .lock()
            .unwrap()
            .as_ref()
            .map(|c| c.snapshot())
    }
}
