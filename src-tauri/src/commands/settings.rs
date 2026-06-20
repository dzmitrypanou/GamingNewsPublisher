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
    let mut settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;

    if vk_oauth::needs_vk_token_refresh(&settings) {
        match vk_oauth::ensure_vk_user_token_fresh(
            &state.http_client(),
            &state.app_handle,
            &mut settings,
        )
        .await
        {
            Ok(()) => {
                let mut result =
                    vk_api::test_connection(&state.http_client(), &settings).await;
                if result.success {
                    result.message = format!(
                        "User token обновлён (VK ID). {}",
                        result.message
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                return Ok(ApiTestResult {
                    success: false,
                    message: format!(
                        "Не удалось обновить VK user token: {e}. \
                         Пройдите «Войти через VK ID» заново."
                    ),
                });
            }
        }
    }

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
pub async fn vk_oauth_start(state: State<'_, std::sync::Arc<AppState>>) -> Result<(), String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let (pending, _) = vk_oauth::begin_oauth(
        &state.app_handle,
        &settings.vk_app_id,
        &settings.vk_service_token,
    )
    .map_err(|e| e.to_string())?;

    *state
        .vk_oauth_pending
        .lock()
        .map_err(|e| e.to_string())? = Some(pending);
    Ok(())
}

#[tauri::command]
pub async fn vk_oauth_finish(
    state: State<'_, std::sync::Arc<AppState>>,
    pasted_url: String,
) -> Result<VkOAuthResult, String> {
    let pending = state
        .vk_oauth_pending
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| {
            "Сначала нажмите «Войти через VK» и пройдите авторизацию в браузере.".to_string()
        })?;

    let mut settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;

    let tokens = vk_oauth::finish_oauth(&state.http_client(), pending, &pasted_url)
        .await
        .map_err(|e| e.to_string())?;

    settings.vk_user_token = tokens.access_token;
    if !tokens.refresh_token.is_empty() {
        settings.vk_refresh_token = tokens.refresh_token;
    }
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;

    let mut message = if settings.vk_refresh_token.is_empty() {
        "User token получен и сохранён.".to_string()
    } else {
        "User token и refresh token получены и сохранены.".to_string()
    };
    if let Some(hint) = vk_api::vk_user_token_photo_hint(&settings.vk_user_token) {
        if settings.vk_refresh_token.is_empty() {
            message = format!("{message} {hint}");
        } else {
            message = format!(
                "{message} Токен VK ID будет автоматически обновляться перед публикацией."
            );
        }
    }

    Ok(VkOAuthResult {
        success: true,
        message,
    })
}

#[tauri::command]
pub async fn vk_legacy_oauth_start(
    state: State<'_, std::sync::Arc<AppState>>,
    app_id: String,
) -> Result<String, String> {
    vk_oauth::begin_legacy_oauth(&state.app_handle, &app_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vk_legacy_oauth_finish(
    state: State<'_, std::sync::Arc<AppState>>,
    pasted_url: String,
) -> Result<VkOAuthResult, String> {
    let access_token = vk_oauth::parse_legacy_token_from_pasted(&pasted_url)
        .map_err(|e| e.to_string())?;

    let mut settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    settings.vk_user_token = access_token.clone();
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;

    let mut message = if access_token.starts_with("vk1.a.") {
        "User token (vk1.a.*) получен и сохранён. Публикация с фото должна работать.".to_string()
    } else if access_token.starts_with("vk2.a.") {
        "Токен сохранён, но формат vk2.a.* не подходит для загрузки фото. \
         Попробуйте другое приложение из списка (Kate Mobile, VK Admin)."
            .to_string()
    } else {
        "User token получен и сохранён.".to_string()
    };

    if let Some(hint) = vk_api::vk_user_token_photo_hint(&access_token) {
        message = format!("{message} {hint}");
    }

    Ok(VkOAuthResult {
        success: !access_token.starts_with("vk2.a."),
        message,
    })
}
