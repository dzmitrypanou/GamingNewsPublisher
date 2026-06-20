mod ai_worker;
mod auto_publish_runtime;
mod auto_publish_scheduler;
mod backup_scheduler;
mod backup_scheduler_runtime;
mod commands;
mod db;
mod fetch;
mod fetch_runtime;
mod fetch_schedule;
mod fetch_scheduler_runtime;
mod local_embed_runtime;
mod local_llm_runtime;
mod models;
mod publish;
mod scheduler;
mod services;

use ai_worker::{start_ai_worker, AiWorkerRuntime};
use auto_publish_runtime::AutoPublishRuntime;
use auto_publish_scheduler::{AutoPublishConfig, AutoPublishSchedulerHandle};
use backup_scheduler::{BackupSchedulerConfig, BackupSchedulerHandle};
use backup_scheduler_runtime::BackupSchedulerRuntime;
use db::Database;
use fetch_runtime::FetchRuntime;
use fetch_scheduler_runtime::FetchSchedulerRuntime;
use local_embed_runtime::LocalEmbedRuntime;
use local_llm_runtime::LocalLlmRuntime;
use scheduler::{FetchConfig, SchedulerHandle};
use services::local_llm_download;
use services::proxy::HttpClientPool;
use services::settings_store;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub app_handle: tauri::AppHandle,
    pub db: Database,
    http_pool: Mutex<HttpClientPool>,
    pub fetch_runtime: FetchRuntime,
    pub fetch_scheduler_runtime: Arc<FetchSchedulerRuntime>,
    pub backup_scheduler_runtime: Arc<BackupSchedulerRuntime>,
    pub auto_publish_runtime: Arc<AutoPublishRuntime>,
    pub ai_worker: Arc<AiWorkerRuntime>,
    pub local_llm: Arc<LocalLlmRuntime>,
    pub local_embed: Arc<LocalEmbedRuntime>,
    scheduler: Mutex<Option<SchedulerHandle>>,
    backup_scheduler: Mutex<Option<BackupSchedulerHandle>>,
    auto_publish_scheduler: Mutex<Option<AutoPublishSchedulerHandle>>,
}

impl AppState {
    pub fn http_client(&self) -> reqwest::Client {
        self.http_pool
            .lock()
            .expect("http pool poisoned")
            .next()
    }

    pub fn rebuild_http_pool(&self, settings: &models::AppSettings) -> anyhow::Result<()> {
        services::proxy::rebuild_pool(&self.http_pool, settings)
    }

    pub fn update_fetch_scheduler(&self, config: FetchConfig) {
        if let Ok(guard) = self.scheduler.lock() {
            if let Some(handle) = guard.as_ref() {
                handle.update(config);
            }
        }
    }

    pub fn update_backup_scheduler(&self, config: BackupSchedulerConfig) {
        if let Ok(guard) = self.backup_scheduler.lock() {
            if let Some(handle) = guard.as_ref() {
                handle.update(config);
            }
        }
    }

    pub fn update_auto_publish_scheduler(&self, config: AutoPublishConfig) {
        if let Ok(guard) = self.auto_publish_scheduler.lock() {
            if let Some(handle) = guard.as_ref() {
                handle.update(config);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let _ = local_llm_download::copy_bundled_server_if_present();

            let data_dir = services::data_dir::resolve(&app_handle)?;
            app_handle
                .asset_protocol_scope()
                .allow_directory(&data_dir, true)
                .map_err(|e| anyhow::anyhow!("asset scope: {}", e))?;
            let db_path = services::data_dir::database_path(&data_dir);
            let database = Database::new(db_path)?;

            let settings = settings_store::load_settings(&app_handle).unwrap_or_default();

            let http_pool = HttpClientPool::from_settings(&settings).unwrap_or_else(|e| {
                eprintln!("HTTP pool init: {}", e);
                HttpClientPool::from_settings(&models::AppSettings {
                    proxy_enabled: false,
                    ..Default::default()
                })
                .expect("direct http client must build")
            });

            let publish_runtime = Arc::new(AutoPublishRuntime::new());
            let fetch_scheduler_runtime = Arc::new(FetchSchedulerRuntime::new());
            let backup_scheduler_runtime = Arc::new(BackupSchedulerRuntime::new());
            let ai_worker = Arc::new(AiWorkerRuntime::new());
            let local_llm = Arc::new(LocalLlmRuntime::new());
            let local_embed = Arc::new(LocalEmbedRuntime::new());

            let state = Arc::new(AppState {
                app_handle: app_handle.clone(),
                db: database,
                http_pool: Mutex::new(http_pool),
                fetch_runtime: FetchRuntime::new(),
                fetch_scheduler_runtime: fetch_scheduler_runtime.clone(),
                backup_scheduler_runtime: backup_scheduler_runtime.clone(),
                auto_publish_runtime: publish_runtime.clone(),
                ai_worker: ai_worker.clone(),
                local_llm: local_llm.clone(),
                local_embed: local_embed.clone(),
                scheduler: Mutex::new(None),
                backup_scheduler: Mutex::new(None),
                auto_publish_scheduler: Mutex::new(None),
            });

            if settings.local_generation_needed()
                && local_llm.is_files_ready(&settings)
            {
                let llm = local_llm.clone();
                let settings = settings.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = llm.start(&settings).await {
                        eprintln!("Local LLM start: {}", e);
                    }
                });
            }

            if settings.local_embed_needed() {
                let dedup_id = settings.normalized_local_dedup_model_id();
                if local_embed.is_files_ready(&dedup_id) {
                    let embed = local_embed.clone();
                    let settings = settings.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = embed.start(&settings, &dedup_id).await {
                            eprintln!("Local embed start: {}", e);
                        }
                    });
                }
            }

            start_ai_worker(state.clone(), ai_worker);

            let scheduler = scheduler::start_scheduler(
                state.clone(),
                fetch_scheduler_runtime,
                FetchConfig::from_settings(&settings),
            );

            if let Ok(mut guard) = state.scheduler.lock() {
                *guard = Some(scheduler);
            }

            let backup_scheduler = backup_scheduler::start_backup_scheduler(
                state.clone(),
                backup_scheduler_runtime,
                BackupSchedulerConfig::from_settings(&settings),
            );

            if let Ok(mut guard) = state.backup_scheduler.lock() {
                *guard = Some(backup_scheduler);
            }

            let auto_publish = auto_publish_scheduler::start_auto_publish_scheduler(
                state.clone(),
                publish_runtime,
                AutoPublishConfig::from_settings(&settings),
            );

            if let Ok(mut guard) = state.auto_publish_scheduler.lock() {
                *guard = Some(auto_publish);
            }

            fetch::purge_puzzle_hints_from_queue(&state);

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::test_vk,
            commands::vk_oauth_authorize,
            commands::test_telegram,
            commands::test_deepseek,
            commands::test_proxy,
            commands::pick_proxy_file,
            commands::fetch_proxy_list,
            commands::pick_watermark_file,
            commands::get_watermark_natural_size,
            commands::resolve_local_image_path,
            commands::read_local_image_data_url,
            commands::get_categories,
            commands::update_category,
            commands::get_sources,
            commands::add_source,
            commands::update_source,
            commands::delete_source,
            commands::get_preset_sources,
            commands::add_preset_sources,
            commands::preview_source,
            commands::get_posts,
            commands::get_post,
            commands::update_post,
            commands::refresh_post_source,
            commands::delete_post,
            commands::reprocess_post,
            commands::fetch_news,
            commands::cancel_fetch_news,
            commands::get_automation_status,
            commands::process_post_with_ai,
            commands::publish_post,
            commands::unpublish_post,
            commands::delete_queue_posts,
            commands::regenerate_queue_images,
            commands::reset_all_data,
            commands::pick_backup_directory,
            commands::export_backup_manual,
            commands::import_backup,
            commands::get_dashboard_stats,
            commands::get_publish_history,
            commands::get_published_posts,
            commands::get_recent_published_posts,
            commands::get_duplicates_overview,
            commands::get_local_llm_status,
            commands::get_local_models_overview,
            commands::download_local_server,
            commands::download_local_model,
            commands::cancel_local_model_download,
            commands::pause_local_model_download,
            commands::cancel_local_server_download,
            commands::delete_local_model,
            commands::delete_local_model_partial,
            commands::add_custom_local_model,
            commands::remove_custom_local_model,
            commands::set_local_model,
            commands::set_local_dedup_model,
            commands::start_local_llm_download,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(state) = window.app_handle().try_state::<Arc<AppState>>() {
                    state.local_llm.shutdown();
                    state.local_embed.shutdown();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::ExitRequested { .. } => {
                    if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                        state.local_llm.shutdown();
                        state.local_embed.shutdown();
                    }
                }
                tauri::RunEvent::Exit => {
                    if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                        state.local_llm.shutdown();
                        state.local_embed.shutdown();
                    }
                }
                _ => {}
            }
        });
}
