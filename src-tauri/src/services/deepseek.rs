use crate::local_embed_runtime::LocalEmbedRuntime;
use crate::local_llm_runtime::LocalLlmRuntime;
use crate::models::{AiResponse, ApiTestResult, AppSettings, DuplicateAiAnalysis, Post};
use crate::services::ai::{self, AiTask};
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use std::sync::atomic::{AtomicBool, Ordering};
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
) -> Result<Option<AiDuplicateMatch>> {
    if posts.is_empty() {
        return Ok(None);
    }

    let dedup_concurrency = dedup_concurrency.clamp(1, 10);

    let found = Arc::new(AtomicBool::new(false));
    let result: Arc<Mutex<Option<AiDuplicateMatch>>> = Arc::new(Mutex::new(None));
    let semaphore = Arc::new(Semaphore::new(dedup_concurrency));
    let mut tasks = JoinSet::new();

    for post in posts {
        let kept_title = post.ai_title.as_deref().unwrap_or(&post.raw_title).to_string();
        let kept_description = post
            .ai_text
            .as_deref()
            .unwrap_or(&post.raw_description)
            .to_string();
        let post_id = post.id;
        let client = client.clone();
        let settings = settings.clone();
        let title = title.to_string();
        let description = description.to_string();
        let found = found.clone();
        let result = result.clone();
        let semaphore = semaphore.clone();
        let local_llm = local_llm.clone();
        let local_embed = local_embed.clone();

        tasks.spawn(async move {
            if found.load(Ordering::Relaxed) {
                return;
            }

            let _permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };

            if found.load(Ordering::Relaxed) {
                return;
            }

            let analysis = match compare_news_pair(
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
            };

            if analysis.is_duplicate && !found.swap(true, Ordering::SeqCst) {
                if let Ok(mut guard) = result.lock() {
                    *guard = Some(AiDuplicateMatch {
                        kept_post_id: post_id,
                        kept_title,
                        analysis,
                    });
                }
            }
        });
    }

    while tasks.join_next().await.is_some() {
        if found.load(Ordering::Relaxed) {
            tasks.abort_all();
            break;
        }
    }

    Ok(result.lock().ok().and_then(|g| g.clone()))
}

pub async fn process_news(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
    title: &str,
    description: &str,
    category: &str,
) -> Result<AiResponse> {
    call_api(client, settings, local_llm, title, description, category).await
}

async fn call_api(
    client: &Client,
    settings: &AppSettings,
    local_llm: &LocalLlmRuntime,
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

    let system = format!(
        "Ты помощник для перевода и написания коротких игровых новостей. \
         Всегда переводи исходный текст на {language}, если он на другом языке. \
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
        return Ok(parsed);
    }
    if let Some(parsed) = extract_ai_response_loose(json_str) {
        return Ok(parsed);
    }
    serde_json::from_str(json_str)
        .context(format!("Failed to parse AI JSON: {}", preview_for_error(json_str)))
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
        .map(|m| unescape_json_string(m.as_str()))?;
    let text = TEXT_FIELD_RE
        .captures(json_str)
        .and_then(|c| c.get(1))
        .map(|m| unescape_json_string(m.as_str()))?;
    if title.trim().is_empty() && text.trim().is_empty() {
        return None;
    }
    let hashtags = HASHTAGS_FIELD_RE
        .captures(json_str)
        .and_then(|c| c.get(1))
        .and_then(|m| serde_json::from_str::<Vec<String>>(m.as_str()).ok())
        .unwrap_or_default();
    Some(AiResponse { title, text, hashtags })
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
