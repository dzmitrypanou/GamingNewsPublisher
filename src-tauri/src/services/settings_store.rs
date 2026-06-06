use crate::models::AppSettings;
use crate::services::data_dir;
use anyhow::Result;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub fn load_settings(app: &AppHandle) -> Result<AppSettings> {
    let data_dir = data_dir::resolve(app)?;
    let store = app.store(data_dir::settings_path(&data_dir))?;
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
        auto_fetch: store
            .get("auto_fetch")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        fetch_interval_minutes: store
            .get("fetch_interval_minutes")
            .and_then(|v| v.as_u64())
            .unwrap_or(30) as u32,
        fetch_items_per_source: store
            .get("fetch_items_per_source")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32,
        auto_publish: store
            .get("auto_publish")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        auto_publish_interval_minutes: store
            .get("auto_publish_interval_minutes")
            .and_then(|v| v.as_u64())
            .unwrap_or(60) as u32,
        auto_publish_jitter_seconds_min: store
            .get("auto_publish_jitter_seconds_min")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0),
        auto_publish_jitter_seconds_max: store
            .get("auto_publish_jitter_seconds_max")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .or_else(|| {
                store
                    .get("auto_publish_jitter_minutes")
                    .and_then(|v| v.as_u64())
                    .map(|v| (v * 60) as u32)
            })
            .unwrap_or(60),
        auto_ai_process: store
            .get("auto_ai_process")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        auto_approve: store
            .get("auto_approve")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        ai_duplicate_check: store
            .get("ai_duplicate_check")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        post_language: store
            .get("post_language")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "ru".to_string()),
    };
    Ok(settings)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<()> {
    let data_dir = data_dir::resolve(app)?;
    let store = app.store(data_dir::settings_path(&data_dir))?;
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
    store.set("auto_fetch", serde_json::json!(settings.auto_fetch));
    store.set(
        "fetch_interval_minutes",
        serde_json::json!(settings.fetch_interval_minutes),
    );
    store.set(
        "fetch_items_per_source",
        serde_json::json!(settings.fetch_items_per_source),
    );
    store.set("auto_publish", serde_json::json!(settings.auto_publish));
    store.set(
        "auto_publish_interval_minutes",
        serde_json::json!(settings.auto_publish_interval_minutes),
    );
    store.set(
        "auto_publish_jitter_seconds_min",
        serde_json::json!(settings.auto_publish_jitter_seconds_min),
    );
    store.set(
        "auto_publish_jitter_seconds_max",
        serde_json::json!(settings.auto_publish_jitter_seconds_max),
    );
    store.set("auto_ai_process", serde_json::json!(settings.auto_ai_process));
    store.set("auto_approve", serde_json::json!(settings.auto_approve));
    store.set(
        "ai_duplicate_check",
        serde_json::json!(settings.ai_duplicate_check),
    );
    store.set("post_language", serde_json::json!(settings.post_language));
    store.save()?;
    Ok(())
}
