use crate::models::FetchResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub struct FetchRuntime {
    is_fetching: AtomicBool,
    last_result: Mutex<Option<FetchResult>>,
}

impl FetchRuntime {
    pub fn new() -> Self {
        Self {
            is_fetching: AtomicBool::new(false),
            last_result: Mutex::new(None),
        }
    }

    pub fn try_begin(&self) -> bool {
        self.is_fetching
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn finish(&self, result: FetchResult) {
        *self.last_result.lock().unwrap() = Some(result);
        self.is_fetching.store(false, Ordering::SeqCst);
    }

    pub fn is_fetching(&self) -> bool {
        self.is_fetching.load(Ordering::SeqCst)
    }

    pub fn last_result(&self) -> Option<FetchResult> {
        self.last_result.lock().unwrap().clone()
    }
}
