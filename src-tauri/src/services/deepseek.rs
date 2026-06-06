use crate::models::{AiResponse, ApiTestResult, AppSettings};
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;

pub async fn test_connection(client: &Client, settings: &AppSettings) -> ApiTestResult {
    if settings.deepseek_api_key.is_empty() {
        return ApiTestResult {
            success: false,
            message: "API ключ не указан".to_string(),
        };
    }

    match call_api(
        client,
        settings,
        "Тест",
        "Тестовое описание новости",
        "PC",
    )
    .await
    {
        Ok(_) => ApiTestResult {
            success: true,
            message: "Подключение успешно".to_string(),
        },
        Err(e) => ApiTestResult {
            success: false,
            message: format!("Ошибка: {}", e),
        },
    }
}

pub async fn process_news(
    client: &Client,
    settings: &AppSettings,
    title: &str,
    description: &str,
    category: &str,
) -> Result<AiResponse> {
    call_api(client, settings, title, description, category).await
}

async fn call_api(
    client: &Client,
    settings: &AppSettings,
    title: &str,
    description: &str,
    category: &str,
) -> Result<AiResponse> {
    let prompt = settings
        .ai_prompt_template
        .replace("{title}", title)
        .replace("{description}", description)
        .replace("{category}", category);

    let body = json!({
        "model": settings.deepseek_model,
        "messages": [
            {
                "role": "system",
                "content": "Ты помощник для написания коротких игровых новостей. Отвечай только валидным JSON."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.7,
        "response_format": { "type": "json_object" }
    });

    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .header("Authorization", format!("Bearer {}", settings.deepseek_api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("DeepSeek request failed")?;

    if !response.status().is_success() {
        let err_text = response.text().await.unwrap_or_default();
        anyhow::bail!("DeepSeek API error: {}", err_text);
    }

    let json: serde_json::Value = response.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .context("No content in DeepSeek response")?;

    parse_ai_response(content)
}

fn parse_ai_response(content: &str) -> Result<AiResponse> {
    let trimmed = content.trim();
    let json_str = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };

    let parsed: AiResponse = serde_json::from_str(json_str)
        .context(format!("Failed to parse AI JSON: {}", json_str))?;
    Ok(parsed)
}

pub fn format_hashtags(tags: &[String]) -> String {
    tags.iter()
        .map(|t| {
            if t.starts_with('#') {
                t.clone()
            } else {
                format!("#{}", t)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
