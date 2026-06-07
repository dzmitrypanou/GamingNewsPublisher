use crate::models::AppSettings;
use crate::services::ai;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

const DEEPSEEK_URL: &str = "https://api.deepseek.com/chat/completions";

pub async fn chat_completions(
    client: &Client,
    settings: &AppSettings,
    body: Value,
) -> Result<String> {
    let response = client
        .post(DEEPSEEK_URL)
        .header(
            "Authorization",
            format!("Bearer {}", settings.deepseek_api_key),
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("DeepSeek request failed")?;

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        anyhow::bail!("DeepSeek API error: {}", err_text);
    }

    let json: Value = response.json().await?;
    ai::extract_message_content(&json)
}
