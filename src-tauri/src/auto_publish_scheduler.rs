use crate::auto_publish_runtime::AutoPublishRuntime;
use crate::publish;
use crate::services::settings_store;
use crate::AppState;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq)]
pub struct AutoPublishConfig {
    pub enabled: bool,
    pub interval_minutes: u32,
    pub jitter_seconds_min: u32,
    pub jitter_seconds_max: u32,
}

impl AutoPublishConfig {
    pub fn from_settings(settings: &crate::models::AppSettings) -> Self {
        Self {
            enabled: settings.auto_publish,
            interval_minutes: settings.auto_publish_interval_minutes.max(1),
            jitter_seconds_min: settings.auto_publish_jitter_seconds_min,
            jitter_seconds_max: settings.auto_publish_jitter_seconds_max,
        }
    }
}

pub struct AutoPublishSchedulerHandle {
    config_tx: watch::Sender<AutoPublishConfig>,
}

impl AutoPublishSchedulerHandle {
    pub fn update(&self, config: AutoPublishConfig) {
        let _ = self.config_tx.send(config);
    }
}

pub fn delay_with_jitter(interval_minutes: u32, jitter_from: u32, jitter_to: u32) -> Duration {
    let interval_minutes = interval_minutes.max(1);
    let from = jitter_from.min(jitter_to);
    let to = jitter_from.max(jitter_to);
    let extra_secs = if from == to {
        from
    } else {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        from + (seed % (to - from + 1) as u64) as u32
    };
    Duration::from_secs(interval_minutes as u64 * 60 + extra_secs as u64)
}

pub fn start_auto_publish_scheduler(
    state: Arc<AppState>,
    runtime: Arc<AutoPublishRuntime>,
    initial: AutoPublishConfig,
) -> AutoPublishSchedulerHandle {
    let (config_tx, mut config_rx) = watch::channel(initial);
    let config_tx_clone = config_tx.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            let config = config_rx.borrow().clone();

            if !config.enabled {
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

            let sleep_duration = delay_with_jitter(
                config.interval_minutes,
                config.jitter_seconds_min,
                config.jitter_seconds_max,
            );
            runtime.set_next(sleep_duration);

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {
                    let current = settings_store::load_settings(&state.app_handle)
                        .map(|s| AutoPublishConfig::from_settings(&s))
                        .unwrap_or_else(|_| config_rx.borrow().clone());

                    if !current.enabled {
                        continue;
                    }

                    match state.db.get_next_publishable_post() {
                        Ok(Some(post)) => {
                            if let Err(e) = publish::do_publish(&state, post.id).await {
                                eprintln!("Auto-publish {}: {}", post.id, e);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("Auto-publish queue error: {}", e),
                    }
                }
                result = config_rx.changed() => {
                    if result.is_err() {
                        break;
                    }
                }
            }
        }
    });

    AutoPublishSchedulerHandle {
        config_tx: config_tx_clone,
    }
}
