mod commands;
mod db;
mod fetch;
mod models;
mod publish;
mod scheduler;
mod services;

use db::Database;
use reqwest::Client;
use scheduler::SchedulerHandle;
use services::settings_store;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub app_handle: tauri::AppHandle,
    pub db: Database,
    pub http_client: Client,
    scheduler: Mutex<Option<SchedulerHandle>>,
}

impl AppState {
    pub fn update_scheduler_interval(&self, minutes: u32) {
        if let Ok(guard) = self.scheduler.lock() {
            if let Some(handle) = guard.as_ref() {
                handle.update_interval(minutes);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&data_dir)?;

            let db_path = data_dir.join("gaming_news.db");
            let database = Database::new(db_path)?;

            let http_client = Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?;

            let settings = settings_store::load_settings(&app_handle)
                .unwrap_or_default();

            let state = Arc::new(AppState {
                app_handle: app_handle.clone(),
                db: database,
                http_client,
                scheduler: Mutex::new(None),
            });

            let scheduler = scheduler::start_scheduler(
                state.clone(),
                settings.fetch_interval_minutes,
            );

            if let Ok(mut guard) = state.scheduler.lock() {
                *guard = Some(scheduler);
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
            commands::process_post_with_ai,
            commands::publish_post,
            commands::get_dashboard_stats,
            commands::get_publish_history,
            commands::get_published_posts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
