use crate::models::PublishResult;
use crate::services::{settings_store, telegram_api, vk_api};
use crate::AppState;
use anyhow::Result;
use chrono::Utc;

pub async fn do_publish(state: &AppState, id: i64) -> Result<PublishResult> {
    let mut post = state.db.get_post(id)?;
    let settings = settings_store::load_settings(&state.app_handle)?;

    let title = post
        .ai_title
        .as_deref()
        .unwrap_or(&post.raw_title);
    let text = post
        .ai_text
        .as_deref()
        .unwrap_or(&post.raw_description);
    let hashtags = post.ai_hashtags.as_deref().unwrap_or("");

    let vk_message = vk_api::format_message(title, text, hashtags);
    let tg_caption = telegram_api::format_caption(title, text, hashtags);
    let image_url = post.raw_image_url.as_deref();

    let mut vk_success = false;
    let mut vk_message_result = String::new();
    let mut tg_success = false;
    let mut tg_message_result = String::new();

    match vk_api::publish_post(
        &state.http_client,
        &settings,
        &vk_message,
        image_url,
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
        &state.http_client,
        &settings,
        &tg_caption,
        image_url,
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
        post.error_message = None;
    } else {
        post.status = "failed".to_string();
        post.error_message = Some(format!("VK: {}; TG: {}", vk_message_result, tg_message_result));
    }

    state.db.update_post(&post)?;

    Ok(PublishResult {
        vk_success,
        vk_message: vk_message_result,
        telegram_success: tg_success,
        telegram_message: tg_message_result,
    })
}
