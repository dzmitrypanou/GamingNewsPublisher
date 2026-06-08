use crate::backup_scheduler_runtime::BackupSchedulerRuntime;
use crate::fetch_schedule::{self, FetchScheduleConfig};
use crate::models::{AppSettings, BackupExportResult};
use crate::services::{backup, data_dir, settings_store};
use crate::AppState;
use chrono::Local;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSchedulerConfig {
    pub schedule: FetchScheduleConfig,
    pub directory: String,
}

impl BackupSchedulerConfig {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            schedule: FetchScheduleConfig {
                enabled: settings.backup_enabled,
                start_at: settings.backup_schedule_start_at.clone(),
                repeat_unit: if settings.backup_repeat_unit.trim().is_empty() {
                    "days".to_string()
                } else {
                    settings.backup_repeat_unit.clone()
                },
                repeat_every: settings.backup_repeat_every.max(1),
            },
            directory: settings.backup_directory.clone(),
        }
    }
}

pub struct BackupSchedulerHandle {
    config_tx: watch::Sender<BackupSchedulerConfig>,
}

impl BackupSchedulerHandle {
    pub fn update(&self, config: BackupSchedulerConfig) {
        let _ = self.config_tx.send(config);
    }
}

pub fn start_backup_scheduler(
    state: Arc<AppState>,
    runtime: Arc<BackupSchedulerRuntime>,
    initial: BackupSchedulerConfig,
) -> BackupSchedulerHandle {
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
            let last_backup_at = state
                .db
                .get_last_backup_at()
                .ok()
                .flatten()
                .and_then(|raw| fetch_schedule::parse_last_fetch_at(&raw));

            let Some((next_at, sleep_duration)) =
                fetch_schedule::compute_next_fetch(&config.schedule, now, last_backup_at)
            else {
                runtime.clear();
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            };

            runtime.set_next(next_at, sleep_duration);

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {
                    let current = settings_store::load_settings(&state.app_handle)
                        .map(|s| BackupSchedulerConfig::from_settings(&s))
                        .unwrap_or_else(|_| config_rx.borrow().clone());

                    if !current.schedule.enabled {
                        continue;
                    }

                    match run_scheduled_backup(&state, &current.directory) {
                        Ok(result) => {
                            let _ = state.db.set_last_backup_at();
                            eprintln!(
                                "Scheduled backup saved: {} ({} bytes)",
                                result.path, result.size_bytes
                            );
                        }
                        Err(e) => eprintln!("Scheduled backup error: {e}"),
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

    BackupSchedulerHandle {
        config_tx: config_tx_clone,
    }
}

fn run_scheduled_backup(state: &AppState, directory: &str) -> anyhow::Result<BackupExportResult> {
    let data_dir = data_dir::resolve(&state.app_handle)?;
    state.db.checkpoint_wal()?;
    let filename = backup::default_backup_filename();
    let backup_dir = data_dir::resolve_backup_directory(&state.app_handle, directory)?;
    let dest = backup_dir.join(filename);
    backup::export_backup(&data_dir, &dest)
}
