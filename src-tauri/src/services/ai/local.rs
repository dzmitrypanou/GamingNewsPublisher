use crate::local_llm_runtime::LocalLlmRuntime;
use crate::models::AppSettings;
use crate::services::ai;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

pub async fn chat_completions(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    body: Value,
) -> Result<String> {
    local_llm.ensure_running(settings).await?;
    let response = client
        .post(local_llm.chat_completions_url())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Local LLM request failed")?;

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Local LLM error: {}", err_text);
    }

    let json: Value = response.json().await?;
    ai::extract_message_content(&json)
}
