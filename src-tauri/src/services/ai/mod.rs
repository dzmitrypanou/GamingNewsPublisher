use crate::local_embed_runtime::LocalEmbedRuntime;
use crate::local_llm_runtime::LocalLlmRuntime;
use crate::models::AppSettings;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

pub mod cloud;
pub mod local;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTask {
    Generation,
    Duplicate,
}

fn provider_for(task: AiTask, settings: &AppSettings) -> &str {
    match task {
        AiTask::Generation => settings.ai_generation_provider.as_str(),
        AiTask::Duplicate => settings.ai_duplicate_provider.as_str(),
    }
}

fn is_configured_for(
    task: AiTask,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    _local_embed: &LocalEmbedRuntime,
) -> bool {
    match provider_for(task, settings) {
        "local" => local_llm.is_files_ready(settings),
        "cloud" => !settings.deepseek_api_key.is_empty(),
        _ => false,
    }
}

fn is_available_for(
    task: AiTask,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    _local_embed: &LocalEmbedRuntime,
) -> bool {
    match provider_for(task, settings) {
        "local" => local_llm.is_ready(settings),
        "cloud" => !settings.deepseek_api_key.is_empty(),
        _ => false,
    }
}

pub fn ai_is_configured_for_generation(
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
) -> bool {
    is_configured_for(AiTask::Generation, settings, local_llm, local_embed)
}

pub fn ai_is_configured_for_duplicate(
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
) -> bool {
    is_configured_for(AiTask::Duplicate, settings, local_llm, local_embed)
}

pub fn ai_is_available_for_generation(
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
) -> bool {
    is_available_for(AiTask::Generation, settings, local_llm, local_embed)
}

pub fn ai_is_available_for_duplicate(
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
) -> bool {
    is_available_for(AiTask::Duplicate, settings, local_llm, local_embed)
}

pub fn ai_is_configured(
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
) -> bool {
    ai_is_configured_for_generation(settings, local_llm, local_embed)
        || ai_is_configured_for_duplicate(settings, local_llm, local_embed)
}

pub fn ai_is_available(
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
) -> bool {
    ai_is_available_for_generation(settings, local_llm, local_embed)
        || ai_is_available_for_duplicate(settings, local_llm, local_embed)
}

pub async fn chat_completions(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    task: AiTask,
    body: Value,
) -> Result<String> {
    match provider_for(task, settings) {
        "local" => local::chat_completions(client, settings, local_llm, body).await,
        "cloud" => cloud::chat_completions(client, settings, body).await,
        _ => bail!("AI provider is disabled for this task"),
    }
}

pub async fn chat_json(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    task: AiTask,
    system: &str,
    user: &str,
    temperature: f32,
    model: &str,
    max_tokens: u32,
) -> Result<String> {
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
        "response_format": { "type": "json_object" }
    });

    let content = chat_completions(client, settings, local_llm, task, body).await?;
    Ok(content)
}

pub fn extract_message_content(json: &Value) -> Result<String> {
    json["choices"][0]["message"]["content"]
        .as_str()
        .context("No content in AI response")
        .map(String::from)
}
