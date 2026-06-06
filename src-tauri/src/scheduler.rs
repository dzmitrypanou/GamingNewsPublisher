use crate::fetch;
use crate::models::AppSettings;
use crate::AppState;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq)]
pub struct FetchConfig {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl FetchConfig {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            enabled: settings.auto_fetch,
            interval_minutes: settings.fetch_interval_minutes.max(5),
        }
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

pub fn start_scheduler(state: Arc<AppState>, initial: FetchConfig) -> SchedulerHandle {
    let (config_tx, mut config_rx) = watch::channel(initial.clone());
    let config_tx_clone = config_tx.clone();

    tauri::async_runtime::spawn(async move {
        if initial.enabled {
            let state_startup = state.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if let Err(e) = fetch::do_fetch(&state_startup).await {
                    if !e.to_string().contains("уже выполняется") {
                        eprintln!("Startup fetch error: {}", e);
                    }
                }
            });
        }

        loop {
            let config = config_rx.borrow().clone();

            if !config.enabled {
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

            let sleep_duration =
                Duration::from_secs(config.interval_minutes.max(5) as u64 * 60);

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {
                    let current = config_rx.borrow().clone();
                    if !current.enabled {
                        continue;
                    }

                    let state_clone = state.clone();
                    tauri::async_runtime::spawn(async move {
                        match fetch::do_fetch(&state_clone).await {
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
