use crate::models::AppSettings;
use crate::services::{data_dir, llm_dir, local_model_catalog};
use anyhow::Result;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

fn resolve_active_model_id(model_id: &str) -> String {
    let id = local_model_catalog::normalize_model_id(model_id);
    if local_model_catalog::llm_model_selectable(id) && llm_dir::model_installed(id) {
        return id.to_string();
    }
    let default = local_model_catalog::default_model_id();
    if llm_dir::model_installed(default) {
        return default.to_string();
    }
    llm_dir::installed_model_ids()
        .into_iter()
        .find(|installed| local_model_catalog::llm_model_selectable(installed))
        .unwrap_or_else(|| default.to_string())
}

fn normalize_backdrop_logo_offset(raw: u32, padding: u32) -> u32 {
    let slack = padding.saturating_mul(2);
    if slack == 0 {
        return 0;
    }
    let value = if raw <= 100 && raw % 50 == 0 {
        ((slack as u64 * raw as u64) / 100) as u32
    } else {
        raw
    };
    value.min(slack)
}

pub fn load_settings(app: &AppHandle) -> Result<AppSettings> {
    let data_dir = data_dir::resolve(app)?;
    let store = app.store(data_dir::settings_path(&data_dir))?;
    let watermark_backdrop_padding = store
        .get("watermark_backdrop_padding")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(14)
        .clamp(0, 80);
    let settings = AppSettings {
        vk_token: store
            .get("vk_token")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        vk_user_token: store
            .get("vk_user_token")
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
        ai_provider: store
            .get("ai_provider")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "cloud".to_string()),
        ai_generation_provider: store
            .get("ai_generation_provider")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| {
                store
                    .get("ai_provider")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "cloud".to_string())
            }),
        ai_duplicate_provider: store
            .get("ai_duplicate_provider")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| {
                store
                    .get("ai_provider")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "cloud".to_string())
            }),
        local_model_id: store
            .get("local_model_id")
            .and_then(|v| v.as_str().map(String::from))
            .map(|id| {
                crate::services::local_model_catalog::normalize_model_id(&id).to_string()
            })
            .unwrap_or_else(|| crate::services::local_model_catalog::default_model_id().to_string()),
        local_dedup_model_id: store
            .get("local_dedup_model_id")
            .and_then(|v| v.as_str().map(String::from))
            .map(|id| {
                crate::services::local_model_catalog::normalize_model_id(&id).to_string()
            })
            .unwrap_or_else(|| {
                crate::services::local_model_catalog::default_dedup_model_id().to_string()
            }),
        local_llm_device: store
            .get("local_llm_device")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "gpu".to_string()),
        local_llm_gpu_layers: store
            .get("local_llm_gpu_layers")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(28)
            .clamp(1, 99),
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
        fetch_schedule_start_at: store
            .get("fetch_schedule_start_at")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        fetch_repeat_unit: store
            .get("fetch_repeat_unit")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "minutes".to_string()),
        fetch_repeat_every: store
            .get("fetch_repeat_every")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or_else(|| {
                store
                    .get("fetch_interval_minutes")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .unwrap_or(30)
            })
            .max(1),
        fetch_items_per_source: store
            .get("fetch_items_per_source")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32,
        fetch_sources_concurrency: store
            .get("fetch_sources_concurrency")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(6)
            .clamp(1, 20),
        fetch_items_concurrency: store
            .get("fetch_items_concurrency")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(4)
            .clamp(1, 16),
        ai_dedup_concurrency: store
            .get("ai_dedup_concurrency")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(2)
            .clamp(1, 10),
        ai_process_concurrency: store
            .get("ai_process_concurrency")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(3)
            .clamp(1, 10),
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
        proxy_enabled: store
            .get("proxy_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        proxy_type: store
            .get("proxy_type")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "http".to_string()),
        proxy_list: store
            .get("proxy_list")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        post_image_width: store
            .get("post_image_width")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(1280)
            .clamp(320, 4096),
        post_image_height: store
            .get("post_image_height")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(720)
            .clamp(180, 4096),
        watermark_enabled: store
            .get("watermark_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        watermark_image: store
            .get("watermark_image")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        watermark_opacity: store
            .get("watermark_opacity")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(85)
            .clamp(0, 100),
        watermark_scale_percent: store
            .get("watermark_scale_percent")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(18)
            .clamp(5, 80),
        watermark_position_mode: store
            .get("watermark_position_mode")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "preset".to_string()),
        watermark_preset: store
            .get("watermark_preset")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "bottom_right".to_string()),
        watermark_margin_x: store
            .get("watermark_margin_x")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(24),
        watermark_margin_y: store
            .get("watermark_margin_y")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(24),
        watermark_x: store
            .get("watermark_x")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0),
        watermark_y: store
            .get("watermark_y")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0),
        watermark_size_mode: store
            .get("watermark_size_mode")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "scale".to_string()),
        watermark_width_px: store
            .get("watermark_width_px")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0),
        watermark_height_px: store
            .get("watermark_height_px")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0),
        watermark_backdrop: store
            .get("watermark_backdrop")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "none".to_string()),
        watermark_backdrop_opacity: store
            .get("watermark_backdrop_opacity")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(65)
            .clamp(0, 100),
        watermark_backdrop_padding,
        watermark_backdrop_color: store
            .get("watermark_backdrop_color")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "#000000".to_string()),
        watermark_backdrop_logo_x: normalize_backdrop_logo_offset(
            store
                .get("watermark_backdrop_logo_x")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(watermark_backdrop_padding),
            watermark_backdrop_padding,
        ),
        watermark_backdrop_logo_y: normalize_backdrop_logo_offset(
            store
                .get("watermark_backdrop_logo_y")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(watermark_backdrop_padding),
            watermark_backdrop_padding,
        ),
        fetch_full_article_text: store
            .get("fetch_full_article_text")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        web_context_enabled: store
            .get("web_context_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        web_search_provider: store
            .get("web_search_provider")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "article_only".to_string()),
        tavily_api_key: store
            .get("tavily_api_key")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        ai_duplicate_window_days: store
            .get("ai_duplicate_window_days")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(30),
        ai_duplicate_check_limit: store
            .get("ai_duplicate_check_limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(200)
            .clamp(10, 1000),
        ai_duplicate_llm_top_k: store
            .get("ai_duplicate_llm_top_k")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(20)
            .clamp(1, 100),
        backup_enabled: store
            .get("backup_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        backup_schedule_start_at: store
            .get("backup_schedule_start_at")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        backup_repeat_unit: store
            .get("backup_repeat_unit")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "days".to_string()),
        backup_repeat_every: store
            .get("backup_repeat_every")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(1)
            .max(1),
        backup_directory: {
            let raw = store
                .get("backup_directory")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "backup".to_string());
            if raw.trim().is_empty() {
                "backup".to_string()
            } else {
                raw
            }
        },
    };
    let loaded_id = settings.local_model_id.clone();
    let resolved_id = resolve_active_model_id(&loaded_id);
    let mut settings = settings;
    let mut needs_save = false;
    if resolved_id != loaded_id {
        settings.local_model_id = resolved_id;
        needs_save = true;
    }
    if settings.local_dedup_model_id != settings.local_model_id {
        settings.local_dedup_model_id = settings.local_model_id.clone();
        needs_save = true;
    }
    if needs_save {
        save_settings(app, &settings)?;
    }
    Ok(settings)
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<()> {
    let data_dir = data_dir::resolve(app)?;
    let store = app.store(data_dir::settings_path(&data_dir))?;
    store.set("vk_token", serde_json::json!(settings.vk_token));
    store.set("vk_user_token", serde_json::json!(settings.vk_user_token));
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
        "ai_provider",
        serde_json::json!(settings.ai_generation_provider),
    );
    store.set(
        "ai_generation_provider",
        serde_json::json!(settings.ai_generation_provider),
    );
    store.set(
        "ai_duplicate_provider",
        serde_json::json!(settings.ai_duplicate_provider),
    );
    store.set("local_model_id", serde_json::json!(settings.local_model_id));
    store.set(
        "local_dedup_model_id",
        serde_json::json!(settings.local_dedup_model_id),
    );
    store.set("local_llm_device", serde_json::json!(settings.local_llm_device));
    store.set(
        "local_llm_gpu_layers",
        serde_json::json!(settings.local_llm_gpu_layers.clamp(1, 99)),
    );
    store.set(
        "ai_prompt_template",
        serde_json::json!(settings.ai_prompt_template),
    );
    store.set("auto_fetch", serde_json::json!(settings.auto_fetch));
    store.set(
        "fetch_interval_minutes",
        serde_json::json!(settings.fetch_interval_minutes.max(1)),
    );
    store.set(
        "fetch_schedule_start_at",
        serde_json::json!(settings.fetch_schedule_start_at),
    );
    store.set("fetch_repeat_unit", serde_json::json!(settings.fetch_repeat_unit));
    store.set(
        "fetch_repeat_every",
        serde_json::json!(settings.fetch_repeat_every.max(1)),
    );
    store.set(
        "fetch_items_per_source",
        serde_json::json!(settings.fetch_items_per_source),
    );
    store.set(
        "fetch_sources_concurrency",
        serde_json::json!(settings.fetch_sources_concurrency.clamp(1, 20)),
    );
    store.set(
        "fetch_items_concurrency",
        serde_json::json!(settings.fetch_items_concurrency.clamp(1, 16)),
    );
    store.set(
        "ai_dedup_concurrency",
        serde_json::json!(settings.ai_dedup_concurrency.clamp(1, 10)),
    );
    store.set(
        "ai_process_concurrency",
        serde_json::json!(settings.ai_process_concurrency.clamp(1, 10)),
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
    store.set("proxy_enabled", serde_json::json!(settings.proxy_enabled));
    store.set("proxy_type", serde_json::json!(settings.proxy_type));
    store.set("proxy_list", serde_json::json!(settings.proxy_list));
    store.set(
        "post_image_width",
        serde_json::json!(settings.post_image_width.clamp(320, 4096)),
    );
    store.set(
        "post_image_height",
        serde_json::json!(settings.post_image_height.clamp(180, 4096)),
    );
    store.set("watermark_enabled", serde_json::json!(settings.watermark_enabled));
    store.set("watermark_image", serde_json::json!(settings.watermark_image));
    store.set(
        "watermark_opacity",
        serde_json::json!(settings.watermark_opacity.clamp(0, 100)),
    );
    store.set(
        "watermark_scale_percent",
        serde_json::json!(settings.watermark_scale_percent.clamp(5, 80)),
    );
    store.set(
        "watermark_position_mode",
        serde_json::json!(settings.watermark_position_mode),
    );
    store.set("watermark_preset", serde_json::json!(settings.watermark_preset));
    store.set("watermark_margin_x", serde_json::json!(settings.watermark_margin_x));
    store.set("watermark_margin_y", serde_json::json!(settings.watermark_margin_y));
    store.set("watermark_x", serde_json::json!(settings.watermark_x));
    store.set("watermark_y", serde_json::json!(settings.watermark_y));
    store.set(
        "watermark_size_mode",
        serde_json::json!(settings.watermark_size_mode),
    );
    store.set(
        "watermark_width_px",
        serde_json::json!(settings.watermark_width_px),
    );
    store.set(
        "watermark_height_px",
        serde_json::json!(settings.watermark_height_px),
    );
    store.set("watermark_backdrop", serde_json::json!(settings.watermark_backdrop));
    store.set(
        "watermark_backdrop_opacity",
        serde_json::json!(settings.watermark_backdrop_opacity.clamp(0, 100)),
    );
    store.set(
        "watermark_backdrop_padding",
        serde_json::json!(settings.watermark_backdrop_padding.clamp(0, 80)),
    );
    store.set(
        "watermark_backdrop_color",
        serde_json::json!(settings.watermark_backdrop_color),
    );
    let backdrop_logo_max = settings.watermark_backdrop_padding.saturating_mul(2);
    store.set(
        "watermark_backdrop_logo_x",
        serde_json::json!(settings.watermark_backdrop_logo_x.clamp(0, backdrop_logo_max)),
    );
    store.set(
        "watermark_backdrop_logo_y",
        serde_json::json!(settings.watermark_backdrop_logo_y.clamp(0, backdrop_logo_max)),
    );
    store.set(
        "fetch_full_article_text",
        serde_json::json!(settings.fetch_full_article_text),
    );
    store.set(
        "web_context_enabled",
        serde_json::json!(settings.web_context_enabled),
    );
    store.set(
        "web_search_provider",
        serde_json::json!(settings.web_search_provider),
    );
    store.set("tavily_api_key", serde_json::json!(settings.tavily_api_key));
    store.set(
        "ai_duplicate_window_days",
        serde_json::json!(settings.ai_duplicate_window_days),
    );
    store.set(
        "ai_duplicate_check_limit",
        serde_json::json!(settings.ai_duplicate_check_limit.clamp(10, 1000)),
    );
    store.set(
        "ai_duplicate_llm_top_k",
        serde_json::json!(settings.ai_duplicate_llm_top_k.clamp(1, 100)),
    );
    store.set("backup_enabled", serde_json::json!(settings.backup_enabled));
    store.set(
        "backup_schedule_start_at",
        serde_json::json!(settings.backup_schedule_start_at),
    );
    store.set("backup_repeat_unit", serde_json::json!(settings.backup_repeat_unit));
    store.set(
        "backup_repeat_every",
        serde_json::json!(settings.backup_repeat_every.max(1)),
    );
    store.set("backup_directory", serde_json::json!(settings.backup_directory));
    store.save()?;
    Ok(())
}
