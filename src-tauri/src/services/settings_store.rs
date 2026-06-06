use crate::models::AppSettings;
use anyhow::Result;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "settings.json";

pub fn load_settings(app: &AppHandle) -> Result<AppSettings> {
    let store = app.store(STORE_PATH)?;
    let settings = AppSettings {
        vk_token: store
            .get("vk_token")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        vk_group_id: store
            .get("vk_group_id")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        telegram_bot_token: store
            .get("telegram_bot_token")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        telegram_channel_id: store
            .get("telegram_channel_id")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        deepseek_api_key: store
            .get("deepseek_api_key")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        deepseek_model: store
            .get("deepseek_model")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "deepseek-chat".to_string()),
        ai_prompt_template: store
            .get("ai_prompt_template")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| crate::models::DEFAULT_PROMPT.to_string()),
        fetch_interval_minutes: store
            .get("fetch_interval_minutes")
            .and_then(|v| v.as_u64())
            .unwrap_or(30) as u32,
        auto_publish: store
            .get("auto_publish")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        auto_ai_process: store
            .get("auto_ai_process")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        post_language: store
            .get("post_language")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "ru".to_string()),
    };
    Ok(settings)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<()> {
    let store = app.store(STORE_PATH)?;
    store.set("vk_token", serde_json::json!(settings.vk_token));
    store.set("vk_group_id", serde_json::json!(settings.vk_group_id));
    store.set(
        "telegram_bot_token",
        serde_json::json!(settings.telegram_bot_token),
    );
    store.set(
        "telegram_channel_id",
        serde_json::json!(settings.telegram_channel_id),
    );
    store.set(
        "deepseek_api_key",
        serde_json::json!(settings.deepseek_api_key),
    );
    store.set("deepseek_model", serde_json::json!(settings.deepseek_model));
    store.set(
        "ai_prompt_template",
        serde_json::json!(settings.ai_prompt_template),
    );
    store.set(
        "fetch_interval_minutes",
        serde_json::json!(settings.fetch_interval_minutes),
    );
    store.set("auto_publish", serde_json::json!(settings.auto_publish));
    store.set("auto_ai_process", serde_json::json!(settings.auto_ai_process));
    store.set("post_language", serde_json::json!(settings.post_language));
    store.save()?;
    Ok(())
}
