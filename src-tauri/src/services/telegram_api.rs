use crate::models::{ApiTestResult, AppSettings};
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

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
) -> Result<String> {
    let chat_id = &settings.telegram_channel_id;
    let truncated = truncate_caption(caption, 1024);

    if let Some(img_url) = image_url {
        match publish_with_photo(client, settings, chat_id, &truncated, img_url).await {
            Ok(id) => return Ok(id),
            Err(_) => {
                // fallback to message
            }
        }
    }

    publish_message(client, settings, chat_id, &truncated).await
}

async fn publish_with_photo(
    client: &Client,
    settings: &AppSettings,
    chat_id: &str,
    caption: &str,
    image_url: &str,
) -> Result<String> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendPhoto",
        settings.telegram_bot_token
    );

    let body = json!({
        "chat_id": chat_id,
        "photo": image_url,
        "caption": caption
    });

    let resp: serde_json::Value = client.post(&url).json(&body).send().await?.json().await?;

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
    let mut parts = vec![title.to_string(), String::new(), text.to_string()];
    if !hashtags.is_empty() {
        parts.push(String::new());
        parts.push(hashtags.to_string());
    }
    parts.join("\n")
}
