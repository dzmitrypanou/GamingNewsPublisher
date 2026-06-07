mod auto_publish_runtime;
mod auto_publish_scheduler;
mod commands;
mod db;
mod fetch;
mod fetch_runtime;
mod models;
mod publish;
mod scheduler;
mod services;

use auto_publish_runtime::AutoPublishRuntime;
use auto_publish_scheduler::{AutoPublishConfig, AutoPublishSchedulerHandle};
use db::Database;
use fetch_runtime::FetchRuntime;
use scheduler::{FetchConfig, SchedulerHandle};
use services::proxy::HttpClientPool;
use services::settings_store;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub app_handle: tauri::AppHandle,
    pub db: Database,
    http_pool: Mutex<HttpClientPool>,
    pub fetch_runtime: FetchRuntime,
    pub auto_publish_runtime: Arc<AutoPublishRuntime>,
    scheduler: Mutex<Option<SchedulerHandle>>,
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

            let data_dir = services::data_dir::resolve(&app_handle)?;
            app_handle
                .asset_protocol_scope()
                .allow_directory(&data_dir, true)
                .map_err(|e| anyhow::anyhow!("asset scope: {}", e))?;
            let db_path = services::data_dir::database_path(&data_dir);
            let database = Database::new(db_path)?;

            let settings = settings_store::load_settings(&app_handle)
                .unwrap_or_default();

            let http_pool = HttpClientPool::from_settings(&settings)
                .unwrap_or_else(|e| {
                    eprintln!("HTTP pool init: {}", e);
                    HttpClientPool::from_settings(&models::AppSettings {
                        proxy_enabled: false,
                        ..Default::default()
                    })
                    .expect("direct http client must build")
                });

            let publish_runtime = Arc::new(AutoPublishRuntime::new());

            let state = Arc::new(AppState {
                app_handle: app_handle.clone(),
                db: database,
                http_pool: Mutex::new(http_pool),
                fetch_runtime: FetchRuntime::new(),
                auto_publish_runtime: publish_runtime.clone(),
                scheduler: Mutex::new(None),
                auto_publish_scheduler: Mutex::new(None),
            });

            let scheduler = scheduler::start_scheduler(
                state.clone(),
                FetchConfig::from_settings(&settings),
            );

            if let Ok(mut guard) = state.scheduler.lock() {
                *guard = Some(scheduler);
            }

            let auto_publish = auto_publish_scheduler::start_auto_publish_scheduler(
                state.clone(),
                publish_runtime,
                AutoPublishConfig::from_settings(&settings),
            );

            if let Ok(mut guard) = state.auto_publish_scheduler.lock() {
                *guard = Some(auto_publish);
            }

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::test_vk,
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
            commands::delete_post,
            commands::fetch_news,
            commands::get_automation_status,
            commands::process_post_with_ai,
            commands::publish_post,
            commands::unpublish_post,
            commands::delete_queue_posts,
            commands::reset_all_data,
            commands::get_dashboard_stats,
            commands::get_publish_history,
            commands::get_published_posts,
            commands::get_recent_published_posts,
            commands::get_duplicates_overview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
