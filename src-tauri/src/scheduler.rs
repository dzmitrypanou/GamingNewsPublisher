use crate::fetch;
use crate::fetch_schedule::{self, FetchScheduleConfig};
use crate::fetch_scheduler_runtime::FetchSchedulerRuntime;
use crate::AppState;
use chrono::Local;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchConfig {
    pub schedule: FetchScheduleConfig,
}

impl FetchConfig {
    pub fn from_settings(settings: &crate::models::AppSettings) -> Self {
        Self {
            schedule: FetchScheduleConfig::from_settings(settings),
        }
    }

    pub fn interval_minutes(&self) -> u32 {
        self.schedule.interval_minutes()
    }
}

pub struct SchedulerHandle {
    config_tx: watch::Sender<FetchConfig>,
}

impl SchedulerHandle {
    pub fn update(&self, config: FetchConfig) {
        let _ = self.config_tx.send(config);
    }
}

pub fn start_scheduler(
    state: Arc<AppState>,
    runtime: Arc<FetchSchedulerRuntime>,
    initial: FetchConfig,
) -> SchedulerHandle {
    let (config_tx, mut config_rx) = watch::channel(initial);
    let config_tx_clone = config_tx.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            let config = config_rx.borrow().clone();

            if !config.schedule.enabled {
                runtime.clear();
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                    result = config_rx.changed() => {
                        if result.is_err() {
                            break;
                        }
                        continue;
                    }
                }
                continue;
            }

            let now = Local::now();
            let last_fetch_at = state
                .db
                .get_dashboard_stats()
                .ok()
                .and_then(|stats| stats.last_fetch_at)
                .and_then(|raw| fetch_schedule::parse_last_fetch_at(&raw));

            let Some((next_at, sleep_duration)) =
                fetch_schedule::compute_next_fetch(&config.schedule, now, last_fetch_at)
            else {
                runtime.clear();
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            };

            runtime.set_next(next_at, sleep_duration);

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {
                    let current = config_rx.borrow().clone();
                    if !current.schedule.enabled {
                        continue;
                    }

                    let state_clone = state.clone();
                    tauri::async_runtime::spawn(async move {
                        match fetch::do_fetch(state_clone).await {
                            Ok(_) => {}
                            Err(e) if e.to_string().contains("уже выполняется") => {}
                            Err(e) => eprintln!("Scheduled fetch error: {}", e),
                        }
                    });
                }
                result = config_rx.changed() => {
                    if result.is_err() {
                        break;
                    }
                }
            }
        }
    });

    SchedulerHandle {
        config_tx: config_tx_clone,
    }
}
