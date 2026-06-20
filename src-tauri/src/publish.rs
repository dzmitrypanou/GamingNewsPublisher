use crate::models::{PublishResult, UnpublishResult};
use crate::services::{ai, content_filter, data_dir, dedup_pipeline, post_text, settings_store, telegram_api, vk_api, vk_oauth};
use crate::AppState;
use anyhow::Result;
use chrono::Utc;

pub async fn do_publish(state: &AppState, id: i64) -> Result<PublishResult> {
    let mut post = state.db.get_post(id)?;
    let mut settings = settings_store::load_settings(&state.app_handle)?;

    if vk_oauth::needs_vk_token_refresh(&settings) {
        if let Err(e) = vk_oauth::ensure_vk_user_token_fresh(
            &state.http_client(),
            &state.app_handle,
            &mut settings,
        )
        .await
        {
            anyhow::bail!("VK: {e}");
        }
    }

    let title = post
        .ai_title
        .as_deref()
        .unwrap_or(&post.raw_title);

    if content_filter::is_hints_or_puzzle_answer_content(title, &post.source_url)
        || content_filter::is_hints_or_puzzle_answer_content(&post.raw_title, &post.source_url)
    {
        let _ = state.db.forget_post(id);
        anyhow::bail!("Пост про подсказки/ответы к ежедневным головоломкам — пропущен");
    }

    if settings.ai_duplicate_check
        && ai::ai_is_configured_for_duplicate(&settings, &state.local_llm, &state.local_embed)
    {
        let description = post
            .ai_text
            .as_deref()
            .unwrap_or(&post.raw_description);

        if let Ok(Some(dup)) = dedup_pipeline::check_duplicate(
            state,
            &settings,
            title,
            description,
            dedup_pipeline::DedupCheckOptions {
                exclude_post_id: Some(id),
                status_filter: Some("published".to_string()),
                should_cancel: None,
            },
        )
        .await
        {
            anyhow::bail!(
                "Дубликат (AI): уже опубликован пост «{}» (id {}). {}",
                dup.kept_title,
                dup.kept_post_id,
                dup.analysis.explanation
            );
        }
    }

    let text = post_text::format_post_text(
        post.ai_text
            .as_deref()
            .unwrap_or(&post.raw_description),
    );
    let title = post_text::strip_links_single_line(title);
    let hashtags = post.ai_hashtags.as_deref().unwrap_or("");

    let vk_message = vk_api::format_message(&title, &text, hashtags);
    let tg_caption = telegram_api::format_caption(&title, &text, hashtags);
    let image_url = post.raw_image_url.as_deref();
    let app_data_dir = data_dir::resolve(&state.app_handle).ok();

    let mut vk_success = false;
    let mut vk_message_result = String::new();
    let mut tg_success = false;
    let mut tg_message_result = String::new();

    match vk_api::publish_post(
        &state.http_client(),
        &settings,
        &vk_message,
        image_url,
        Some(&post.source_url),
        app_data_dir.as_deref(),
    )
    .await
    {
        Ok(post_id) => {
            vk_success = true;
            vk_message_result = format!("Опубликовано: post #{}", post_id);
            post.vk_post_id = Some(post_id);
            let _ = state.db.add_publish_log(id, "vk", true, &vk_message_result);
        }
        Err(e) => {
            vk_message_result = e.to_string();
            let _ = state.db.add_publish_log(id, "vk", false, &vk_message_result);
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    match telegram_api::publish_post(
        &state.http_client(),
        &settings,
        &tg_caption,
        image_url,
        app_data_dir.as_deref(),
    )
    .await
    {
        Ok(msg_id) => {
            tg_success = true;
            tg_message_result = format!("Опубликовано: message #{}", msg_id);
            post.telegram_message_id = Some(msg_id);
            let _ = state.db.add_publish_log(id, "telegram", true, &tg_message_result);
        }
        Err(e) => {
            tg_message_result = e.to_string();
            let _ = state.db.add_publish_log(id, "telegram", false, &tg_message_result);
        }
    }

    if vk_success || tg_success {
        post.status = "published".to_string();
        post.published_at = Some(Utc::now().to_rfc3339());
        if vk_success && tg_success {
            post.error_message = None;
        } else if !vk_success {
            post.error_message = Some(format!("VK: {vk_message_result}"));
        } else {
            post.error_message = Some(format!("Telegram: {tg_message_result}"));
        }
    } else {
        post.status = "failed".to_string();
        post.error_message = Some(format!("VK: {vk_message_result}; TG: {tg_message_result}"));
    }

    state.db.update_post(&post)?;

    Ok(PublishResult {
        vk_success,
        vk_message: vk_message_result,
        telegram_success: tg_success,
        telegram_message: tg_message_result,
    })
}

pub async fn do_unpublish(state: &AppState, id: i64) -> Result<UnpublishResult> {
    let mut post = state.db.get_post(id)?;
    let settings = settings_store::load_settings(&state.app_handle)?;

    let has_vk = post.vk_post_id.is_some();
    let has_tg = post.telegram_message_id.is_some();

    if !has_vk && !has_tg {
        anyhow::bail!("Пост не опубликован в VK или Telegram");
    }

    let mut vk_success = !has_vk;
    let mut vk_message = if has_vk {
        String::new()
    } else {
        "Не публиковался в VK".to_string()
    };
    let mut tg_success = !has_tg;
    let mut tg_message = if has_tg {
        String::new()
    } else {
        "Не публиковался в Telegram".to_string()
    };

    if let Some(vk_post_id) = post.vk_post_id.as_deref() {
        match vk_api::delete_post(&state.http_client(), &settings, vk_post_id).await {
            Ok(()) => {
                vk_success = true;
                vk_message = format!("Удалено: post #{}", vk_post_id);
                let _ = state.db.add_publish_log(id, "vk", true, &vk_message);
            }
            Err(e) => {
                vk_message = e.to_string();
                let _ = state.db.add_publish_log(id, "vk", false, &vk_message);
            }
        }
    }

    if let Some(tg_msg_id) = post.telegram_message_id.as_deref() {
        match telegram_api::delete_message(&state.http_client(), &settings, tg_msg_id).await {
            Ok(()) => {
                tg_success = true;
                tg_message = format!("Удалено: message #{}", tg_msg_id);
                let _ = state.db.add_publish_log(id, "telegram", true, &tg_message);
            }
            Err(e) => {
                tg_message = e.to_string();
                let _ = state.db.add_publish_log(id, "telegram", false, &tg_message);
            }
        }
    }

    if vk_success && tg_success {
        post.vk_post_id = None;
        post.telegram_message_id = None;
        post.published_at = None;
        post.error_message = None;
        post.status = if post.ai_title.is_some() {
            "approved".to_string()
        } else {
            "new".to_string()
        };
        state.db.update_post(&post)?;
    }

    Ok(UnpublishResult {
        vk_success,
        vk_message,
        telegram_success: tg_success,
        telegram_message: tg_message,
    })
}
