use crate::models::{ApiTestResult, AppSettings};
use crate::services::image_loader;
use crate::services::post_text;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::path::Path;

pub async fn test_connection(client: &Client, settings: &AppSettings) -> ApiTestResult {
    if settings.telegram_bot_token.is_empty() || settings.telegram_channel_id.is_empty() {
        return ApiTestResult {
            success: false,
            message: "Токен бота или ID канала не указаны".to_string(),
        };
    }

    match get_bot_info(client, settings).await {
        Ok(name) => ApiTestResult {
            success: true,
            message: format!("Бот подключён: @{}", name),
        },
        Err(e) => ApiTestResult {
            success: false,
            message: format!("Ошибка: {}", e),
        },
    }
}

async fn get_bot_info(client: &Client, settings: &AppSettings) -> Result<String> {
    let url = format!(
        "https://api.telegram.org/bot{}/getMe",
        settings.telegram_bot_token
    );
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            resp["description"].as_str().unwrap_or("Telegram API error")
        );
    }

    Ok(resp["result"]["username"]
        .as_str()
        .unwrap_or("bot")
        .to_string())
}

pub async fn publish_post(
    client: &Client,
    settings: &AppSettings,
    caption: &str,
    image_url: Option<&str>,
    data_dir: Option<&Path>,
) -> Result<String> {
    let chat_id = &settings.telegram_channel_id;
    let truncated = truncate_caption(caption, 1024);

    if let Some(img_url) = image_url {
        match publish_with_photo(client, settings, chat_id, &truncated, img_url, data_dir).await {
            Ok(id) => return Ok(id),
            Err(_) => {

            }
        }
    }

    publish_message(client, settings, chat_id, &truncated).await
}

pub async fn delete_message(
    client: &Client,
    settings: &AppSettings,
    message_id: &str,
) -> Result<()> {
    let url = format!(
        "https://api.telegram.org/bot{}/deleteMessage",
        settings.telegram_bot_token
    );

    let body = json!({
        "chat_id": settings.telegram_channel_id,
        "message_id": message_id.parse::<i64>().context("Invalid message_id")?,
    });

    let resp: serde_json::Value = client.post(&url).json(&body).send().await?.json().await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            resp["description"].as_str().unwrap_or("deleteMessage error")
        );
    }

    Ok(())
}

async fn publish_with_photo(
    client: &Client,
    settings: &AppSettings,
    chat_id: &str,
    caption: &str,
    image_url: &str,
    data_dir: Option<&Path>,
) -> Result<String> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendPhoto",
        settings.telegram_bot_token
    );

    let resp: serde_json::Value = if image_loader::is_local_image_ref(image_url) {
        let dir = data_dir.context("data_dir required for local images")?;
        let bytes = image_loader::load_image_bytes(client, dir, image_url).await?;
        let form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .text("parse_mode", "HTML".to_string())
            .text("caption", caption.to_string())
            .part(
                "photo",
                reqwest::multipart::Part::bytes(bytes)
                    .file_name("photo.jpg")
                    .mime_str("image/jpeg")?,
            );
        client.post(&url).multipart(form).send().await?.json().await?
    } else {
        let body = json!({
            "chat_id": chat_id,
            "photo": image_url,
            "caption": caption,
            "parse_mode": "HTML"
        });
        client.post(&url).json(&body).send().await?.json().await?
    };

    if !resp["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            resp["description"].as_str().unwrap_or("sendPhoto error")
        );
    }

    Ok(resp["result"]["message_id"]
        .as_i64()
        .context("No message_id")?
        .to_string())
}

async fn publish_message(
    client: &Client,
    settings: &AppSettings,
    chat_id: &str,
    text: &str,
) -> Result<String> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        settings.telegram_bot_token
    );

    let body = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML",
        "disable_web_page_preview": false
    });

    let resp: serde_json::Value = client.post(&url).json(&body).send().await?.json().await?;

    if !resp["ok"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            resp["description"].as_str().unwrap_or("sendMessage error")
        );
    }

    Ok(resp["result"]["message_id"]
        .as_i64()
        .context("No message_id")?
        .to_string())
}

fn truncate_caption(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max - 1).collect::<String>() + "…"
}

pub fn format_caption(title: &str, text: &str, hashtags: &str) -> String {
    let title_esc = post_text::escape_html(title);
    let hashtags_esc = if hashtags.is_empty() {
        String::new()
    } else {
        post_text::escape_html(hashtags)
    };

    let header = format!("<b>{title_esc}</b>");
    let footer = if hashtags_esc.is_empty() {
        String::new()
    } else {
        format!("\n\n{hashtags_esc}")
    };

    let overhead = header.chars().count() + footer.chars().count() + if text.is_empty() { 0 } else { 2 };
    let max_text = 1024usize.saturating_sub(overhead);
    let text_esc = post_text::escape_html(text);
    let text_esc = truncate_caption(&text_esc, max_text);

    if text_esc.is_empty() {
        format!("{header}{footer}")
    } else {
        format!("{header}\n\n{text_esc}{footer}")
    }
}
