use crate::models::AppSettings;
use crate::services::rss_fetcher;
use anyhow::{Context, Result};
use regex::Regex;
use reqwest::Client;
use serde_json::json;

const MAX_CONTEXT_CHARS: usize = 3000;
const ARTICLE_FETCH_MAX_BYTES: usize = 512 * 1024;

pub async fn build_web_context(
    client: &Client,
    settings: &AppSettings,
    source_url: &str,
    title: &str,
) -> String {
    if !settings.web_context_enabled {
        return String::new();
    }

    let mut parts: Vec<String> = Vec::new();

    if settings.web_search_provider == "article_only" || settings.web_search_provider == "tavily" {
        if let Ok(article) = fetch_article_text(client, source_url).await {
            if !article.is_empty() {
                parts.push(format!("Полный текст статьи с сайта:\n{article}"));
            }
        }
    }

    if settings.web_search_provider == "tavily" && !settings.tavily_api_key.trim().is_empty() {
        match tavily_search(client, settings.tavily_api_key.trim(), title).await {
            Ok(snippet) if !snippet.is_empty() => {
                parts.push(format!("Дополнительный контекст из поиска:\n{snippet}"));
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Tavily search: {}", e);
            }
        }
    }

    if parts.is_empty() {
        return String::new();
    }

    truncate_chars(&parts.join("\n\n"), MAX_CONTEXT_CHARS)
}

async fn fetch_article_text(client: &Client, url: &str) -> Result<String> {
    let url = url.trim();
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
        return Ok(String::new());
    }

    let response = client
        .get(url)
        .header("User-Agent", rss_fetcher::user_agent())
        .header("Accept", "text/html,application/xhtml+xml")
        .send()
        .await
        .context("article fetch failed")?;

    if !response.status().is_success() {
        return Ok(String::new());
    }

    let bytes = response
        .bytes()
        .await
        .context("article body read failed")?;
    if bytes.len() > ARTICLE_FETCH_MAX_BYTES {
        return Ok(String::new());
    }

    let html = String::from_utf8_lossy(&bytes);
    Ok(extract_article_text(&html))
}

pub fn extract_article_text(html: &str) -> String {
    let article_re = Regex::new(r"(?is)<article[^>]*>(.*?)</article>").ok();
    let chunk = if let Some(re) = article_re {
        re.captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or(html)
    } else {
        html
    };

    let p_re = match Regex::new(r"(?is)<p[^>]*>(.*?)</p>") {
        Ok(re) => re,
        Err(_) => return rss_fetcher::clean_html(chunk),
    };

    let paragraphs: Vec<String> = p_re
        .captures_iter(chunk)
        .filter_map(|cap| cap.get(1))
        .map(|m| rss_fetcher::clean_html(m.as_str()))
        .filter(|p| p.len() >= 40)
        .take(12)
        .collect();

    if paragraphs.is_empty() {
        rss_fetcher::clean_html(chunk)
    } else {
        paragraphs.join("\n\n")
    }
}

async fn tavily_search(client: &Client, api_key: &str, query: &str) -> Result<String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(String::new());
    }

    let body = json!({
        "api_key": api_key,
        "query": query,
        "max_results": 3,
        "search_depth": "basic",
        "include_answer": true,
    });

    let response = client
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Tavily request failed")?;

    if !response.status().is_success() {
        let err = response.text().await.unwrap_or_default();
        anyhow::bail!("Tavily API error: {}", err);
    }

    let json: serde_json::Value = response.json().await.context("Tavily JSON invalid")?;
    let mut parts: Vec<String> = Vec::new();

    if let Some(answer) = json.get("answer").and_then(|v| v.as_str()) {
        let answer = answer.trim();
        if !answer.is_empty() {
            parts.push(answer.to_string());
        }
    }

    if let Some(results) = json.get("results").and_then(|v| v.as_array()) {
        for item in results.iter().take(3) {
            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if title.is_empty() && content.is_empty() {
                continue;
            }
            parts.push(format!("{title}: {content}"));
        }
    }

    Ok(truncate_chars(&parts.join("\n"), 1500))
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max).collect::<String>() + "…"
}
