use crate::local_embed_runtime::LocalEmbedRuntime;
use crate::local_llm_runtime::LocalLlmRuntime;
use crate::models::{AiResponse, ApiTestResult, AppSettings, DuplicateAiAnalysis, Post};
use crate::services::ai::{self, AiTask};
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const NEWS_MAX_TOKENS: u32 = 1024;
const DEDUP_MAX_TOKENS: u32 = 256;

pub async fn test_connection(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
) -> ApiTestResult {
    if settings.generation_uses_local() {
        if !local_llm.is_files_ready(settings) {
            return ApiTestResult {
                success: false,
                message: "Локальная модель не загружена".to_string(),
            };
        }
    } else if settings.ai_generation_provider == "off" {
        return ApiTestResult {
            success: false,
            message: "Генерация постов отключена".to_string(),
        };
    } else if settings.deepseek_api_key.is_empty() {
        return ApiTestResult {
            success: false,
            message: "API ключ не указан".to_string(),
        };
    }

    match call_api(
        client,
        settings,
        local_llm,
        "Тест",
        "Тестовое описание новости",
        "PC",
        "",
    )
    .await
    {
        Ok(_) => ApiTestResult {
            success: true,
            message: if settings.generation_uses_local() {
                "Локальная модель работает".to_string()
            } else {
                "Подключение успешно".to_string()
            },
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
    local_llm: &LocalLlmRuntime,
    local_embed: &LocalEmbedRuntime,
    title_a: &str,
    description_a: &str,
    title_b: &str,
    description_b: &str,
) -> Result<DuplicateAiAnalysis> {
    if settings.duplicate_uses_local() && settings.duplicate_uses_embeddings() {
        return crate::services::embedding_dedup::compare_news_pair_embeddings(
            client,
            settings,
            local_embed,
            &settings.normalized_local_dedup_model_id(),
            title_a,
            description_a,
            title_b,
            description_b,
        )
        .await;
    }

    if settings.duplicate_uses_local() && settings.duplicate_uses_llm() {
        let dedup_id = settings.normalized_local_dedup_model_id();
        let _ = local_llm
            .ensure_running_for_model(settings, &dedup_id)
            .await;
    }

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

    let model = settings.effective_duplicate_model();
    let content = ai::chat_json(
        client,
        settings,
        local_llm,
        AiTask::Duplicate,
        "Ты эксперт по сравнению новостей. Отвечай только валидным JSON.",
        &prompt,
        0.2,
        &model,
        DEDUP_MAX_TOKENS,
    )
    .await?;

    parse_duplicate_analysis(&content)
}

#[derive(Clone)]
pub struct AiDuplicateMatch {
    pub kept_post_id: i64,
    pub kept_title: String,
    pub analysis: DuplicateAiAnalysis,
}

pub async fn find_ai_duplicate_among_posts(
    client: &Client,
    settings: &AppSettings,
    local_llm: Arc<LocalLlmRuntime>,
    local_embed: Arc<LocalEmbedRuntime>,
    title: &str,
    description: &str,
    posts: &[Post],
    dedup_concurrency: usize,
    should_cancel: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
) -> Result<Option<AiDuplicateMatch>> {
    if posts.is_empty() {
        return Ok(None);
    }

    let dedup_concurrency = dedup_concurrency.clamp(1, 10);

    let best_match: Arc<Mutex<Option<AiDuplicateMatch>>> = Arc::new(Mutex::new(None));
    let semaphore = Arc::new(Semaphore::new(dedup_concurrency));
    let mut tasks = JoinSet::new();

    let use_embeddings = settings.duplicate_uses_local() && settings.duplicate_uses_embeddings();
    let dedup_model_id = settings.normalized_local_dedup_model_id();

    for post in posts {
        if crate::services::duplicate::is_link_roundup_title(&post.raw_title) {
            continue;
        }

        let kept_title = post.ai_title.as_deref().unwrap_or(&post.raw_title).to_string();
        let kept_description = post
            .ai_text
            .as_deref()
            .unwrap_or(&post.raw_description)
            .to_string();
        let post_id = post.id;
        let raw_title = post.raw_title.clone();
        let raw_description = post.raw_description.clone();
        let ai_title = post.ai_title.clone();
        let ai_text = post.ai_text.clone();
        let client = client.clone();
        let settings = settings.clone();
        let title = title.to_string();
        let description = description.to_string();
        let best_match = best_match.clone();
        let semaphore = semaphore.clone();
        let local_llm = local_llm.clone();
        let local_embed = local_embed.clone();
        let should_cancel = should_cancel.clone();
        let dedup_model_id = dedup_model_id.clone();

        tasks.spawn(async move {
            if should_cancel.as_ref().is_some_and(|f| f()) {
                return;
            }

            let _permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };

            if should_cancel.as_ref().is_some_and(|f| f()) {
                return;
            }

            let analysis = if use_embeddings {
                match crate::services::embedding_dedup::compare_news_to_kept_post_embeddings(
                    &client,
                    &settings,
                    &local_embed,
                    &dedup_model_id,
                    &title,
                    &description,
                    &raw_title,
                    &raw_description,
                    ai_title.as_deref(),
                    ai_text.as_deref(),
                )
                .await
                {
                    Ok(a) => a,
                    Err(_) => return,
                }
            } else {
                match compare_news_pair(
                    &client,
                    &settings,
                    &local_llm,
                    &local_embed,
                    &title,
                    &description,
                    &kept_title,
                    &kept_description,
                )
                .await
                {
                    Ok(a) => a,
                    Err(_) => return,
                }
            };

            if !analysis.is_duplicate {
                return;
            }

            if let Ok(mut guard) = best_match.lock() {
                let replace = guard
                    .as_ref()
                    .map_or(true, |existing| analysis.confidence > existing.analysis.confidence);
                if replace {
                    *guard = Some(AiDuplicateMatch {
                        kept_post_id: post_id,
                        kept_title,
                        analysis,
                    });
                }
            }
        });
    }

    let poll_cancel = should_cancel.clone();
    loop {
        if poll_cancel.as_ref().is_some_and(|f| f()) {
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            break;
        }
        match tokio::time::timeout(
            std::time::Duration::from_millis(200),
            tasks.join_next(),
        )
        .await
        {
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => continue,
        }
    }

    Ok(best_match.lock().ok().and_then(|g| g.clone()))
}

pub async fn process_news(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    title: &str,
    description: &str,
    category: &str,
    source_url: &str,
) -> Result<AiResponse> {
    call_api(
        client,
        settings,
        local_llm,
        title,
        description,
        category,
        source_url,
    )
    .await
}

async fn call_api(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    title: &str,
    description: &str,
    category: &str,
    source_url: &str,
) -> Result<AiResponse> {
    let language = language_label(&settings.post_language).to_string();

    let web_context =
        crate::services::web_context::build_web_context(client, settings, source_url).await;
    let web_context_block = if web_context.is_empty() {
        String::new()
    } else {
        format!("\nДополнительный контекст из интернета:\n{web_context}")
    };

    let prompt = settings
        .ai_prompt_template
        .replace("{title}", title)
        .replace("{description}", description)
        .replace("{category}", category)
        .replace("{language}", &language)
        .replace("{web_context}", &web_context_block);

    let system = format!(
        "Ты помощник для перевода и написания коротких игровых новостей. \
         Если исходный текст на другом языке — переводи на {language}. \
         Если уже на {language} — перепиши живым языком для соцсетей. \
         Не выдумывай факты: опирайся только на данные из запроса. \
         Отвечай только одним валидным JSON-объектом, без markdown и пояснений. \
         Поле text — не длиннее 400 символов. Все текстовые поля — на {language}."
    );

    let model = settings.effective_generation_model();
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..2 {
        let temperature = if attempt == 0 { 0.7 } else { 0.3 };
        let content = match ai::chat_json(
            client,
            settings,
            local_llm,
            AiTask::Generation,
            &system,
            &prompt,
            temperature,
            &model,
            NEWS_MAX_TOKENS,
        )
        .await
        {
            Ok(content) => content,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        match parse_ai_response(&content) {
            Ok(parsed) => return Ok(parsed),
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Не удалось получить ответ AI")))
}

fn parse_duplicate_analysis(content: &str) -> Result<DuplicateAiAnalysis> {
    let json_str = strip_code_fence(content);
    let parsed: DuplicateAiAnalysis = serde_json::from_str(json_str)
        .context(format!("Failed to parse duplicate AI JSON: {}", json_str))?;
    Ok(parsed)
}

fn parse_ai_response(content: &str) -> Result<AiResponse> {
    let json_str = strip_code_fence(content);
    if let Ok(parsed) = serde_json::from_str::<AiResponse>(json_str) {
        return Ok(normalize_ai_response(parsed));
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(parsed) = ai_response_from_value(&value) {
            return Ok(parsed);
        }
    }
    if let Some(parsed) = extract_ai_response_loose(json_str) {
        return Ok(parsed);
    }
    Err(anyhow::anyhow!(
        "Failed to parse AI JSON: {}",
        preview_for_error(json_str)
    ))
}

fn normalize_ai_response(mut response: AiResponse) -> AiResponse {
    response.title = response.title.trim().to_string();
    response.text = response.text.trim().to_string();
    if response.title.is_empty() && !response.text.is_empty() {
        response.title = derive_title_from_text(&response.text);
    }
    if response.text.is_empty() && !response.title.is_empty() {
        response.text = response.title.clone();
    }
    response
}

fn ai_response_from_value(value: &serde_json::Value) -> Option<AiResponse> {
    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let text = value
        .get("text")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let hashtags = value
        .get("hashtags")
        .map(parse_hashtags_value)
        .unwrap_or_default();

    match (title, text) {
        (Some(title), Some(text)) => Some(normalize_ai_response(AiResponse {
            title,
            text,
            hashtags,
        })),
        (None, Some(text)) => Some(normalize_ai_response(AiResponse {
            title: String::new(),
            text,
            hashtags,
        })),
        (Some(title), None) => Some(normalize_ai_response(AiResponse {
            title: title.clone(),
            text: title,
            hashtags,
        })),
        (None, None) => None,
    }
}

fn parse_hashtags_value(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        serde_json::Value::String(raw) => raw
            .split_whitespace()
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn derive_title_from_text(text: &str) -> String {
    let compact: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return String::new();
    }

    let end = compact
        .find(|c| matches!(c, '.' | '!' | '?'))
        .map(|idx| idx + 1)
        .unwrap_or(compact.len());
    let sentence = compact[..end].trim();
    truncate_chars(
        if sentence.is_empty() {
            compact.as_str()
        } else {
            sentence
        },
        80,
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

static TITLE_FIELD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""title"\s*:\s*"((?:\\.|[^"\\])*)""#).unwrap());
static TEXT_FIELD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""text"\s*:\s*"((?:\\.|[^"\\])*)"#).unwrap());
static HASHTAGS_FIELD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""hashtags"\s*:\s*(\[(?:\\.|[^\]])*\])"#).unwrap()
});

fn extract_ai_response_loose(json_str: &str) -> Option<AiResponse> {
    let title = TITLE_FIELD_RE
        .captures(json_str)
        .and_then(|c| c.get(1))
        .map(|m| unescape_json_string(m.as_str()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let text = TEXT_FIELD_RE
        .captures(json_str)
        .and_then(|c| c.get(1))
        .map(|m| unescape_json_string(m.as_str()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let hashtags = HASHTAGS_FIELD_RE
        .captures(json_str)
        .and_then(|c| c.get(1))
        .and_then(|m| serde_json::from_str::<Vec<String>>(m.as_str()).ok())
        .unwrap_or_default();

    match (title, text) {
        (Some(title), Some(text)) => Some(normalize_ai_response(AiResponse {
            title,
            text,
            hashtags,
        })),
        (None, Some(text)) => Some(normalize_ai_response(AiResponse {
            title: String::new(),
            text,
            hashtags,
        })),
        (Some(title), None) => Some(normalize_ai_response(AiResponse {
            title: title.clone(),
            text: title,
            hashtags,
        })),
        (None, None) => None,
    }
}

fn unescape_json_string(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn preview_for_error(content: &str) -> String {
    let compact: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 240 {
        compact
    } else {
        format!("{}…", compact.chars().take(240).collect::<String>())
    }
}

fn strip_code_fence(content: &str) -> &str {
    let trimmed = content.trim();
    if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ai_response_accepts_text_only_json() {
        let raw = r#"{"text": "Внимание, геймеры! Свежая новость: разработчики анонсировали крупное обновление."}"#;
        let parsed = parse_ai_response(raw).expect("text-only json should parse");
        assert!(parsed.title.contains("Внимание, геймеры"));
        assert_eq!(
            parsed.text,
            "Внимание, геймеры! Свежая новость: разработчики анонсировали крупное обновление."
        );
        assert!(parsed.hashtags.is_empty());
    }
}
