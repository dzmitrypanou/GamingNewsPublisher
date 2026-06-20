use crate::auto_publish_scheduler::AutoPublishConfig;
use crate::backup_scheduler::BackupSchedulerConfig;
use crate::scheduler::FetchConfig;
use crate::models::{ApiTestResult, AppSettings, VkOAuthResult};
use crate::services::{deepseek, proxy, settings_store, telegram_api, vk_api, vk_oauth};
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
    Ok(proxy::test_proxy_from_settings(&settings).await)
}

#[tauri::command]
pub async fn vk_oauth_authorize(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<VkOAuthResult, String> {
    let mut settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;

    let tokens = vk_oauth::authorize_user_token(
        &state.app_handle,
        &state.http_client(),
        &settings.vk_app_id,
        &settings.vk_service_token,
    )
    .await
    .map_err(|e| e.to_string())?;

    settings.vk_user_token = tokens.access_token;
    if !tokens.refresh_token.is_empty() {
        settings.vk_refresh_token = tokens.refresh_token;
    }
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;

    Ok(VkOAuthResult {
        success: true,
        message: if settings.vk_refresh_token.is_empty() {
            "User token получен и сохранён.".to_string()
        } else {
            "User token и refresh token получены и сохранены.".to_string()
        },
    })
}
