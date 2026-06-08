use crate::auto_publish_scheduler::AutoPublishConfig;
use crate::backup_scheduler::BackupSchedulerConfig;
use crate::scheduler::FetchConfig;
use crate::models::{ApiTestResult, AppSettings};
use crate::services::{deepseek, proxy, settings_store, telegram_api, vk_api};
use crate::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, Arc<AppState>>,
    settings: AppSettings,
) -> Result<(), String> {
    state
        .rebuild_http_pool(&settings)
        .map_err(|e| e.to_string())?;
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;
    state.update_fetch_scheduler(FetchConfig::from_settings(&settings));
    state.update_backup_scheduler(BackupSchedulerConfig::from_settings(&settings));
    state.update_auto_publish_scheduler(AutoPublishConfig::from_settings(&settings));

    if settings.local_llm_needed() {
        if state.local_llm.is_files_ready(&settings) {
            let llm = state.local_llm.clone();
            let settings = settings.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = llm.start(&settings).await {
                    eprintln!("Local LLM start after settings save: {}", e);
                }
            });
        }
    } else {
        state.local_llm.shutdown();
    }

    Ok(())
}

#[tauri::command]
pub async fn test_vk(state: State<'_, std::sync::Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    Ok(vk_api::test_connection(&state.http_client(), &settings).await)
}

#[tauri::command]
pub async fn test_telegram(state: State<'_, std::sync::Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    Ok(telegram_api::test_connection(&state.http_client(), &settings).await)
}

#[tauri::command]
pub async fn test_deepseek(state: State<'_, Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    if settings.generation_uses_local() && state.local_llm.is_files_ready(&settings) {
        state.local_llm.start(&settings).await.map_err(|e| e.to_string())?;
    }
    Ok(deepseek::test_connection(&state.http_client(), &settings, &state.local_llm).await)
}

#[tauri::command]
pub async fn test_proxy(state: State<'_, std::sync::Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    if !settings.proxy_enabled {
        return Ok(ApiTestResult {
            success: false,
            message: "Включите прокси в настройках".to_string(),
        });
    }
    Ok(proxy::test_proxy_connection(&state.http_client()).await)
}
