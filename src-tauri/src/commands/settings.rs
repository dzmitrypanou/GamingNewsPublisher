use crate::auto_publish_scheduler::AutoPublishConfig;
use crate::scheduler::FetchConfig;
use crate::models::{ApiTestResult, AppSettings};
use crate::services::{deepseek, settings_store, telegram_api, vk_api};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, std::sync::Arc<AppState>>) -> Result<AppSettings, String> {
    settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, std::sync::Arc<AppState>>,
    settings: AppSettings,
) -> Result<(), String> {
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;
    state.update_fetch_scheduler(FetchConfig::from_settings(&settings));
    state.update_auto_publish_scheduler(AutoPublishConfig::from_settings(&settings));
    Ok(())
}

#[tauri::command]
pub async fn test_vk(state: State<'_, std::sync::Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    Ok(vk_api::test_connection(&state.http_client, &settings).await)
}

#[tauri::command]
pub async fn test_telegram(state: State<'_, std::sync::Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    Ok(telegram_api::test_connection(&state.http_client, &settings).await)
}

#[tauri::command]
pub async fn test_deepseek(state: State<'_, std::sync::Arc<AppState>>) -> Result<ApiTestResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    Ok(deepseek::test_connection(&state.http_client, &settings).await)
}
