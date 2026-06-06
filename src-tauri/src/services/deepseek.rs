use crate::models::{AiResponse, ApiTestResult, AppSettings, DuplicateAiAnalysis, Post};
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

pub async fn compare_news_pair(
    client: &Client,
    settings: &AppSettings,
    title_a: &str,
    description_a: &str,
    title_b: &str,
    description_b: &str,
) -> Result<DuplicateAiAnalysis> {
    let desc_a = truncate_for_ai(description_a);
    let desc_b = truncate_for_ai(description_b);
    let prompt = format!(
        "Сравни две игровые новости и определи, описывают ли они одно и то же событие (дубль) \
         или это разные новости.\n\
         Ответ строго JSON:\n\
         {{\n\
           \"is_duplicate\": true или false,\n\
           \"confidence\": число 0-100,\n\
           \"explanation\": \"краткое обоснование на русском, 1-2 предложения\"\n\
         }}\n\n\
         Новость A:\n\
         Заголовок: {title_a}\n\
         Текст: {desc_a}\n\n\
         Новость B:\n\
         Заголовок: {title_b}\n\
         Текст: {desc_b}"
    );

    let body = json!({
        "model": settings.deepseek_model,
        "messages": [
            {
                "role": "system",
                "content": "Ты эксперт по сравнению новостей. Отвечай только валидным JSON."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.2,
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

    parse_duplicate_analysis(content)
}

pub struct AiDuplicateMatch {
    pub kept_post_id: i64,
    pub kept_title: String,
    pub analysis: DuplicateAiAnalysis,
}

pub async fn find_ai_duplicate_among_posts(
    client: &Client,
    settings: &AppSettings,
    title: &str,
    description: &str,
    posts: &[Post],
) -> Result<Option<AiDuplicateMatch>> {
    for post in posts {
        let kept_title = post.ai_title.as_deref().unwrap_or(&post.raw_title);
        let kept_description = post.ai_text.as_deref().unwrap_or(&post.raw_description);
        let analysis = compare_news_pair(
            client,
            settings,
            title,
            description,
            kept_title,
            kept_description,
        )
        .await?;
        if analysis.is_duplicate {
            return Ok(Some(AiDuplicateMatch {
                kept_post_id: post.id,
                kept_title: kept_title.to_string(),
                analysis,
            }));
        }
    }
    Ok(None)
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
    let language = language_label(&settings.post_language).to_string();

    let prompt = settings
        .ai_prompt_template
        .replace("{title}", title)
        .replace("{description}", description)
        .replace("{category}", category)
        .replace("{language}", &language);

    let body = json!({
        "model": settings.deepseek_model,
        "messages": [
            {
                "role": "system",
                "content": format!(
                    "Ты помощник для перевода и написания коротких игровых новостей. \
                     Всегда переводи исходный текст на {language}, если он на другом языке. \
                     Отвечай только валидным JSON. Все текстовые поля — на {language}."
                )
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

fn parse_duplicate_analysis(content: &str) -> Result<DuplicateAiAnalysis> {
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

    let parsed: DuplicateAiAnalysis = serde_json::from_str(json_str)
        .context(format!("Failed to parse duplicate AI JSON: {}", json_str))?;
    Ok(parsed)
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

fn truncate_for_ai(text: &str) -> String {
    let compact: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 500 {
        compact
    } else {
        compact.chars().take(500).collect()
    }
}

fn language_label(code: &str) -> String {
    match code.trim().to_lowercase().as_str() {
        "" => "русский".to_string(),
        "ru" | "rus" | "russian" => "русский".to_string(),
        "en" | "eng" | "english" => "английский".to_string(),
        other => other.to_string(),
    }
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
