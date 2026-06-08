use crate::models::AppSettings;
use crate::services::content_filter;
use crate::services::rss_fetcher;
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

const MAX_CONTEXT_CHARS: usize = 3000;
const MAX_FULL_ARTICLE_CHARS: usize = 20_000;
const ARTICLE_FETCH_TIMEOUT: Duration = Duration::from_secs(90);
const ARTICLE_ENRICH_TIMEOUT: Duration = Duration::from_secs(15);
const ARTICLE_FETCH_MAX_BYTES_CONTEXT: usize = 512 * 1024;
const ARTICLE_FETCH_MAX_BYTES_FULL: usize = 2 * 1024 * 1024;
/// RSS excerpt with at least this many characters is enough during fetch — skip slow page loads.
const MIN_RSS_ENRICH_CHARS: usize = 80;

#[derive(Debug, Clone, Copy)]
pub enum ArticleFetchMode {
    WebContext,
    FullArticle,
    RssEnrich,
}

struct ExtractOptions {
    max_paragraphs: usize,
    min_paragraph_len: usize,
    max_chars: usize,
}

impl ArticleFetchMode {
    fn extract_options(self) -> ExtractOptions {
        match self {
            Self::WebContext => ExtractOptions {
                max_paragraphs: 12,
                min_paragraph_len: 40,
                max_chars: MAX_CONTEXT_CHARS,
            },
            Self::FullArticle | Self::RssEnrich => ExtractOptions {
                max_paragraphs: 80,
                min_paragraph_len: 20,
                max_chars: MAX_FULL_ARTICLE_CHARS,
            },
        }
    }

    fn max_bytes(self) -> usize {
        match self {
            Self::WebContext | Self::RssEnrich => ARTICLE_FETCH_MAX_BYTES_CONTEXT,
            Self::FullArticle => ARTICLE_FETCH_MAX_BYTES_FULL,
        }
    }

    fn request_timeout(self) -> Duration {
        match self {
            Self::RssEnrich => ARTICLE_ENRICH_TIMEOUT,
            _ => ARTICLE_FETCH_TIMEOUT,
        }
    }

    fn max_attempts(self) -> u32 {
        match self {
            Self::RssEnrich => 1,
            _ => 2,
        }
    }
}

fn rss_enrichment_sufficient(rss_clean: &str) -> bool {
    let rss = rss_clean.trim();
    !rss.is_empty()
        && !content_filter::is_navigation_boilerplate(rss)
        && rss.chars().count() >= MIN_RSS_ENRICH_CHARS
}

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
        if let Ok(article) =
            fetch_article_text(client, source_url, ArticleFetchMode::WebContext).await
        {
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

pub async fn enrich_rss_description(
    client: &Client,
    link: &str,
    rss_description: &str,
) -> String {
    let rss_clean = rss_fetcher::strip_boilerplate(rss_description);
    if rss_enrichment_sufficient(&rss_clean) {
        return rss_clean;
    }

    let full = fetch_article_text(client, link, ArticleFetchMode::RssEnrich).await;
    let full_clean = full
        .as_ref()
        .map(|text| rss_fetcher::strip_boilerplate(text))
        .unwrap_or_default();

    pick_best_article_text(&rss_clean, &full_clean)
}

static OG_TITLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<meta[^>]+property=["']og:title["'][^>]+content=["']([^"']+)["']"#)
        .expect("og:title regex")
});
static OG_TITLE_RE_ALT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?is)<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:title["']"#)
        .expect("og:title alt regex")
});
static HTML_TITLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<title[^>]*>([^<]+)</title>").expect("title regex"));

pub fn extract_page_title(html: &str) -> Option<String> {
    let title = OG_TITLE_RE
        .captures(html)
        .or_else(|| OG_TITLE_RE_ALT.captures(html))
        .or_else(|| HTML_TITLE_RE.captures(html))
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim())
        .filter(|t| !t.is_empty())
        .map(|t| rss_fetcher::clean_html(t).trim().to_string())
        .filter(|t| !t.is_empty())?;
    Some(title)
}

pub async fn fetch_page_title(client: &Client, url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
        return None;
    }

    let mut request = client
        .get(url)
        .header("User-Agent", rss_fetcher::user_agent())
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9,ru;q=0.8")
        .timeout(ArticleFetchMode::RssEnrich.request_timeout());

    if let Some(referer) = article_referer(url) {
        request = request.header("Referer", referer);
    }

    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let bytes = response.bytes().await.ok()?;
    let max_bytes = 256 * 1024;
    let html = if bytes.len() > max_bytes {
        String::from_utf8_lossy(&bytes[..max_bytes]).into_owned()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };

    extract_page_title(&html)
}

fn pick_best_article_text(rss: &str, full: &str) -> String {
    let rss = rss.trim();
    let full = full.trim();

    let rss_ok = !rss.is_empty() && !content_filter::is_navigation_boilerplate(rss);
    let full_ok = !full.is_empty() && !content_filter::is_navigation_boilerplate(full);

    if full_ok && full.chars().count() >= 200 {
        return full.to_string();
    }
    if full_ok && rss_ok {
        if full.chars().count() > rss.chars().count() {
            return full.to_string();
        }
        if rss.chars().count() > full.chars().count() + 80 {
            return rss.to_string();
        }
        return full.to_string();
    }
    if full_ok {
        return full.to_string();
    }
    if rss_ok {
        return rss.to_string();
    }
    if !full.is_empty() {
        return full.to_string();
    }
    rss.to_string()
}

pub async fn fetch_article_text(
    client: &Client,
    url: &str,
    mode: ArticleFetchMode,
) -> Result<String> {
    let url = url.trim();
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
        return Ok(String::new());
    }

    let max_attempts = mode.max_attempts();
    for attempt in 0..max_attempts {
        match fetch_article_text_once(client, url, mode).await {
            Ok(text) if !text.trim().is_empty() => return Ok(text),
            Ok(_) if attempt + 1 >= max_attempts => return Ok(String::new()),
            Ok(_) => {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            Err(e) if attempt + 1 >= max_attempts => return Err(e),
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
        }
    }

    Ok(String::new())
}

async fn fetch_article_text_once(
    client: &Client,
    url: &str,
    mode: ArticleFetchMode,
) -> Result<String> {
    let mut request = client
        .get(url)
        .header("User-Agent", rss_fetcher::user_agent())
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "en-US,en;q=0.9,ru;q=0.8")
        .timeout(mode.request_timeout());

    if let Some(referer) = article_referer(url) {
        request = request.header("Referer", referer);
    }

    let response = request.send().await.context("article fetch failed")?;

    if !response.status().is_success() {
        return Ok(String::new());
    }

    let bytes = response
        .bytes()
        .await
        .context("article body read failed")?;
    if bytes.len() > mode.max_bytes() {
        return Ok(String::new());
    }

    let html = String::from_utf8_lossy(&bytes);
    Ok(extract_article_text(&html, mode))
}

pub fn extract_article_text(html: &str, mode: ArticleFetchMode) -> String {
    let options = mode.extract_options();
    let mut candidates: Vec<String> = Vec::new();

    if let Some(body) = extract_json_ld_article_body(html) {
        let cleaned = rss_fetcher::strip_boilerplate(&rss_fetcher::clean_html(&body));
        if !cleaned.is_empty() {
            candidates.push(cleaned);
        }
    }

    if let Some(body) = extract_after_main_heading(html, &options) {
        if !body.is_empty() {
            candidates.push(body);
        }
    }

    let chunk = select_content_chunk(html);
    let paragraphs = extract_paragraphs(chunk, &options);
    let from_html = if paragraphs.is_empty() {
        rss_fetcher::clean_html(chunk)
    } else {
        rss_fetcher::strip_boilerplate(&paragraphs.join("\n\n"))
    };
    if !from_html.is_empty() {
        candidates.push(from_html);
    }

    if let Some(meta) = extract_meta_description(html) {
        candidates.push(meta);
    }

    let text = candidates
        .into_iter()
        .filter(|text| !content_filter::is_navigation_boilerplate(text))
        .max_by_key(|text| text.len())
        .unwrap_or_default();

    truncate_chars(&text, options.max_chars)
}

fn select_content_chunk(html: &str) -> &str {
    if let Some(chunk) = extract_marked_content(html) {
        return chunk;
    }

    if let Ok(re) = Regex::new(r"(?is)<article[^>]*>(.*)</article>") {
        if let Some(cap) = re.captures(html).and_then(|c| c.get(1)) {
            return cap.as_str();
        }
    }

    html
}

fn extract_marked_content(html: &str) -> Option<&str> {
    const START_MARKERS: &[&str] = &[
        r#"id="article-body""#,
        r#"id='article-body'"#,
        r#"id="postBody""#,
        r#"id='postBody'"#,
        r#"class="post__content""#,
        r#"class='post__content'"#,
        r#"class='post__content wysiwyg"#,
        r#"itemprop="articleBody""#,
        r#"class="entry-content"#,
        r#"class="post-content"#,
        r#"class="article-content"#,
        r#"class="article-body"#,
        r#"class="story-body"#,
        r#"class="content-body"#,
        r#"class="paid-content"#,
        r#"class="m-detail-body"#,
        r#"data-component="article-body""#,
        r#"class="text-copy bodyCopy"#,
        r#"class="item-body"#,
        r#"data-cy="article-body""#,
        r#"data-cy="article""#,
        r#"<main"#,
    ];

    const END_MARKERS: &[&str] = &[
        r#"id="slice-container-authorBio""#,
        r#"class="author author__"#,
        r#"<footer"#,
        r#"data-component-name="RelatedArticles""#,
        r#"class="related-articles"#,
        r#"Leave a Reply"#,
        r#"Related Articles"#,
        r#"See also:"#,
        r#"0 comments"#,
        r#"class="comments-area"#,
    ];

    for marker in START_MARKERS {
        let start_idx = html.find(marker)?;
        let after_marker = &html[start_idx..];
        let gt = after_marker.find('>')?;
        let content_start = start_idx + gt + 1;

        for end_marker in END_MARKERS {
            if let Some(rel) = html[content_start..].find(end_marker) {
                if rel > 200 {
                    return Some(&html[content_start..content_start + rel]);
                }
            }
        }

        let end = (content_start + 120_000).min(html.len());
        if end > content_start + 200 {
            return Some(&html[content_start..end]);
        }
    }

    None
}

fn extract_paragraphs(chunk: &str, options: &ExtractOptions) -> Vec<String> {
    let Ok(re) = Regex::new(r"(?is)<(?:p|h2|h3)[^>]*>(.*?)</(?:p|h2|h3)>") else {
        return Vec::new();
    };

    re.captures_iter(chunk)
        .filter_map(|cap| cap.get(1))
        .map(|m| rss_fetcher::clean_html(m.as_str()))
        .filter(|p| p.len() >= options.min_paragraph_len)
        .filter(|p| !rss_fetcher::is_boilerplate_paragraph(p))
        .take(options.max_paragraphs)
        .collect()
}

fn extract_after_main_heading(html: &str, options: &ExtractOptions) -> Option<String> {
    let h1_re = Regex::new(r"(?is)<h1[^>]*>").ok()?;
    let h1_match = h1_re.find(html)?;
    let content_start = h1_match.end();

    const END_MARKERS: &[&str] = &[
        "Related Articles",
        "Leave a Reply",
        "See also:",
        "0 comments",
        "slice-container-authorBio",
        "comments-area",
        "Related Articles",
    ];

    let mut content_end = html.len().min(content_start + 180_000);
    for marker in END_MARKERS {
        if let Some(rel) = html[content_start..content_end].find(marker) {
            if rel > 200 {
                content_end = content_start + rel;
            }
        }
    }

    let region = &html[content_start..content_end];
    let paragraphs = extract_paragraphs(region, options);
    if paragraphs.len() < 2 {
        return None;
    }

    Some(rss_fetcher::strip_boilerplate(&paragraphs.join("\n\n")))
}

fn article_referer(url: &str) -> Option<String> {
    let re = Regex::new(r"^(https?://[^/]+)").ok()?;
    re.captures(url)
        .map(|cap| format!("{}/", &cap[1]))
}

fn extract_meta_description(html: &str) -> Option<String> {
    let patterns = [
        r#"(?is)<meta[^>]+property=["']og:description["'][^>]+content=["']([^"']+)["']"#,
        r#"(?is)<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:description["']"#,
        r#"(?is)<meta[^>]+name=["']description["'][^>]+content=["']([^"']+)["']"#,
        r#"(?is)<meta[^>]+content=["']([^"']+)["'][^>]+name=["']description["']"#,
    ];

    for pattern in patterns {
        let Ok(re) = Regex::new(pattern) else {
            continue;
        };
        if let Some(cap) = re.captures(html) {
            let desc = rss_fetcher::strip_boilerplate(&rss_fetcher::clean_html(&cap[1]));
            if desc.len() >= 40 {
                return Some(desc);
            }
        }
    }

    None
}

fn extract_json_ld_article_body(html: &str) -> Option<String> {
    let re = Regex::new(r#"(?is)<script[^>]+type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
        .ok()?;

    for cap in re.captures_iter(html) {
        let raw = cap.get(1)?.as_str().trim();
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
            if let Some(body) = article_body_from_json_ld(&value) {
                return Some(body);
            }
        }
    }

    None
}

fn article_body_from_json_ld(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => items.iter().find_map(article_body_from_json_ld),
        serde_json::Value::Object(map) => {
            if let Some(graph) = map.get("@graph").and_then(|g| g.as_array()) {
                if let Some(body) = graph.iter().find_map(article_body_from_json_ld) {
                    return Some(body);
                }
            }

            if json_ld_is_article_type(map.get("@type")) {
                if let Some(body) = map.get("articleBody").and_then(|v| v.as_str()) {
                    let body = body.trim();
                    if !body.is_empty() {
                        return Some(body.to_string());
                    }
                }
                if let Some(desc) = map.get("description").and_then(|v| v.as_str()) {
                    let desc = desc.trim();
                    if desc.chars().count() >= 120 {
                        return Some(desc.to_string());
                    }
                }
            }

            None
        }
        _ => None,
    }
}

fn json_ld_is_article_type(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::String(kind)) => {
            let lower = kind.to_ascii_lowercase();
            lower.contains("article") || lower.contains("blogposting")
        }
        Some(serde_json::Value::Array(kinds)) => kinds.iter().any(|kind| {
            kind.as_str()
                .map(|s| {
                    let lower = s.to_ascii_lowercase();
                    lower.contains("article") || lower.contains("blogposting")
                })
                .unwrap_or(false)
        }),
        _ => false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_article_text_from_article_body_marker() {
        let html = r#"
            <html><body>
            <div id="article-body" class="text-copy">
                <p>Short</p>
                <p>While there's never any shortage of games releasing on Steam any given day, the release calendar feels thin.</p>
                <p>Until this week everything changed for the release calendar.</p>
                <p>Thanks to the showcase reveals, I've gone from wondering to worrying about my wishlist.</p>
            </div>
            <div id="slice-container-authorBio">Author bio</div>
            </body></html>
        "#;
        let text = extract_article_text(html, ArticleFetchMode::FullArticle);
        assert!(text.contains("shortage of games"));
        assert!(text.contains("Until this week everything"));
        assert!(!text.contains("Author bio"));
    }

    #[test]
    fn extract_article_text_from_json_ld() {
        let html = r#"
            <html><head>
            <script type="application/ld+json">
            {"@context":"https://schema.org","@type":"NewsArticle","articleBody":"<p>Manufacturing expanded for the third month.</p><p>Supply chains remained under pressure.</p>"}
            </script>
            </head><body><nav>Topics Archive Blog</nav></body></html>
        "#;
        let text = extract_article_text(html, ArticleFetchMode::FullArticle);
        assert!(text.contains("Manufacturing expanded"));
        assert!(text.contains("Supply chains"));
        assert!(!text.contains("Topics Archive"));
    }

    #[test]
    fn extract_article_text_after_h1() {
        let html = r#"
            <html><body>
            <h1>Geopolitics, AI, and Jensen Huang</h1>
            <p>At Computex 2026 this week, the electronics industry hit a clear inflection point.</p>
            <p>Everywhere you turned, people chased the CEO for selfies and autographs.</p>
            <h3>Taiwan thrives on the nation role</h3>
            <p>After six years of build-up, the conversation has shifted entirely to GPUs.</p>
            <div>Related Articles</div>
            </body></html>
        "#;
        let text = extract_article_text(html, ArticleFetchMode::FullArticle);
        assert!(text.contains("Computex 2026"));
        assert!(text.contains("shifted entirely to GPUs"));
        assert!(!text.contains("Related Articles"));
    }

    #[test]
    fn pick_best_prefers_full_article_text() {
        let rss = "Jensen Huang and AI frenzy steal the show at Computex 2026.";
        let full = "At Computex 2026 this week, the electronics industry hit a clear inflection point. \
Everywhere you turned, people chased the CEO for selfies, autographs, and just to feel his presence.";
        let picked = pick_best_article_text(rss, full);
        assert!(picked.contains("inflection point"));
    }

    #[test]
    fn rss_enrichment_sufficient_skips_short_or_boilerplate() {
        let rss_excerpt = "At Computex 2026 this week, the electronics industry hit a clear inflection point: \
it's the new rock and roll, proved by a mob of attendees chasing the show's ultimate star attraction, Nvidia CEO Jensen";
        assert!(rss_enrichment_sufficient(rss_excerpt));
        assert!(!rss_enrichment_sufficient("Too short excerpt."));
        assert!(!rss_enrichment_sufficient(
            "Physics Mathematics Biology Computer Science Topics Archive Blog Columns"
        ));
    }

    #[test]
    fn extract_article_text_respects_context_limits() {
        let html = r#"<article><p>#"#;
        let mut long = html.to_string();
        for i in 0..20 {
            long.push_str(&format!(
                "<p>This is paragraph number {i} with enough characters to pass the minimum length filter easily.</p>"
            ));
        }
        long.push_str("</article>");
        let text = extract_article_text(&long, ArticleFetchMode::WebContext);
        assert!(text.chars().count() <= MAX_CONTEXT_CHARS + 4);
    }
}
