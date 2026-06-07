use crate::models::{PresetSource, RssPreviewItem};
use anyhow::{Context, Result};
use atom_syndication::Feed as AtomFeed;
use regex::Regex;
use reqwest::Client;
use rss::Channel;
use std::borrow::Cow;

const RSS_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 GamingNewsPublisher/0.1";
const RSS_ACCEPT: &str =
    "application/rss+xml, application/atom+xml, application/xml, text/xml, */*";

pub fn user_agent() -> &'static str {
    RSS_USER_AGENT
}

pub fn get_preset_sources() -> Vec<PresetSource> {
    vec![
        // General Gaming News
        PresetSource {
            name: "IGN".to_string(),
            url: "https://feeds.feedburner.com/ign/all".to_string(),
            category_name: "Обзоры".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "GameSpot".to_string(),
            url: "https://www.gamespot.com/feeds/news/".to_string(),
            category_name: "Обзоры".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "Eurogamer".to_string(),
            url: "https://www.eurogamer.net/feed".to_string(),
            category_name: "Обзоры".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "Kotaku".to_string(),
            url: "https://feeds.feedburner.com/kotaku".to_string(),
            category_name: "Обзоры".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "PC Gamer".to_string(),
            url: "https://www.pcgamer.com/rss/".to_string(),
            category_name: "PC".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "Polygon".to_string(),
            url: "https://www.polygon.com/rss/index.xml".to_string(),
            category_name: "Обзоры".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "Gematsu".to_string(),
            url: "https://feeds.feedburner.com/gematsu".to_string(),
            category_name: "Консоли".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "Rock Paper Shotgun".to_string(),
            url: "https://www.rockpapershotgun.com/feed".to_string(),
            category_name: "Инди".to_string(),
            group: "General Gaming News".to_string(),
        },
        // Industry & Business
        PresetSource {
            name: "GamesIndustry.biz".to_string(),
            url: "https://www.gamesindustry.biz/feed".to_string(),
            category_name: "Анонсы".to_string(),
            group: "Industry & Business".to_string(),
        },
        PresetSource {
            name: "VGC (Video Games Chronicle)".to_string(),
            url: "https://www.videogameschronicle.com/feed/".to_string(),
            category_name: "Анонсы".to_string(),
            group: "Industry & Business".to_string(),
        },
        // Leaks & Rumors
        PresetSource {
            name: "Insider Gaming".to_string(),
            url: "https://insider-gaming.com/feed/".to_string(),
            category_name: "Анонсы".to_string(),
            group: "Leaks & Rumors".to_string(),
        },
        // Hardware & Tech
        PresetSource {
            name: "Tom's Hardware".to_string(),
            url: "https://www.tomshardware.com/feeds/all".to_string(),
            category_name: "PC".to_string(),
            group: "Hardware & Tech".to_string(),
        },
    ]
}

pub struct RssItem {
    pub title: String,
    pub description: String,
    pub link: String,
    pub image_url: Option<String>,
    pub pub_date: Option<String>,
}

pub async fn fetch_rss_items(client: &Client, url: &str, limit: usize) -> Result<Vec<RssItem>> {
    let bytes = fetch_feed_bytes(client, url).await?;
    parse_feed_bytes(&bytes, limit)
}

fn resolve_feed_url(url: &str) -> Cow<'_, str> {
    let normalized = url.trim().trim_end_matches('/');
    match normalized {
        "https://kotaku.com/rss" | "http://kotaku.com/rss" => {
            Cow::Borrowed("https://feeds.feedburner.com/kotaku")
        }
        "https://www.gematsu.com/feed" | "http://www.gematsu.com/feed" => {
            Cow::Borrowed("https://feeds.feedburner.com/gematsu")
        }
        _ => Cow::Borrowed(url),
    }
}

fn is_cloudflare_challenge(status: reqwest::StatusCode, body: &[u8]) -> bool {
    if status != reqwest::StatusCode::FORBIDDEN {
        return false;
    }
    let body_lower = String::from_utf8_lossy(body).to_ascii_lowercase();
    body_lower.contains("challenges.cloudflare.com")
        || body_lower.contains("cf-mitigated")
        || body_lower.contains("just a moment")
}

async fn fetch_feed_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let resolved_url = resolve_feed_url(url);
    let response = client
        .get(resolved_url.as_ref())
        .header("User-Agent", RSS_USER_AGENT)
        .header("Accept", RSS_ACCEPT)
        .send()
        .await
        .context("Не удалось загрузить RSS")?;

    let status = response.status();
    let bytes = response.bytes().await?.to_vec();

    if status.is_success() {
        return Ok(bytes);
    }

    if is_cloudflare_challenge(status, &bytes) {
        anyhow::bail!(
            "Cloudflare заблокировал доступ к ленте ({}). Для этого сайта используйте прокси-URL.",
            status.as_u16()
        );
    }

    anyhow::bail!("Не удалось загрузить RSS: HTTP {}", status.as_u16());
}

fn parse_feed_bytes(bytes: &[u8], limit: usize) -> Result<Vec<RssItem>> {
    if let Ok(channel) = Channel::read_from(bytes) {
        return Ok(parse_rss_channel(&channel, limit));
    }

    if let Ok(feed) = AtomFeed::read_from(bytes) {
        return Ok(parse_atom_feed(&feed, limit));
    }

    anyhow::bail!("Не удалось разобрать RSS или Atom")
}

fn parse_rss_channel(channel: &Channel, limit: usize) -> Vec<RssItem> {
    let mut items = Vec::new();
    for entry in channel.items().iter().take(limit) {
        let title = entry.title().unwrap_or("").to_string();
        let description = clean_html(
            entry
                .description()
                .or_else(|| entry.content())
                .unwrap_or(""),
        );
        let link = entry
            .link()
            .or_else(|| entry.guid().map(|g| g.value()))
            .unwrap_or("")
            .to_string();

        let image_url = extract_image_from_entry(entry);
        let pub_date = entry.pub_date().map(|s| s.to_string());

        if !link.is_empty() {
            items.push(RssItem {
                title,
                description,
                link,
                image_url,
                pub_date,
            });
        }
    }
    items
}

fn parse_atom_feed(feed: &AtomFeed, limit: usize) -> Vec<RssItem> {
    let mut items = Vec::new();

    for entry in feed.entries().iter().take(limit) {
        let title = entry.title().value.clone();

        let link = entry
            .links()
            .iter()
            .find(|l| {
                let rel = l.rel();
                rel.is_empty() || rel == "alternate"
            })
            .or_else(|| entry.links().first())
            .map(|l| l.href().to_string())
            .unwrap_or_default();

        let raw_content = entry
            .content()
            .and_then(|c| c.value.clone())
            .or_else(|| entry.summary().map(|s| s.value.clone()))
            .unwrap_or_default();

        let description = clean_html(&raw_content);
        let image_url = extract_image_from_atom_entry(entry, &raw_content);
        let pub_date = entry
            .published()
            .map(|d| d.to_rfc3339())
            .or_else(|| Some(entry.updated().to_rfc3339()));

        if !link.is_empty() {
            items.push(RssItem {
                title,
                description,
                link,
                image_url,
                pub_date,
            });
        }
    }

    items
}

fn extract_image_from_atom_entry(
    entry: &atom_syndication::Entry,
    raw_content: &str,
) -> Option<String> {
    if let Some(media) = entry.extensions().get("media") {
        if let Some(thumbnail) = media.get("thumbnail") {
            for ext in thumbnail {
                if let Some(url) = ext.attrs().get("url") {
                    return Some(url.to_string());
                }
            }
        }
        if let Some(content) = media.get("content") {
            for ext in content {
                if let Some(url) = ext.attrs().get("url") {
                    return Some(url.to_string());
                }
            }
        }
    }

    extract_img_from_html(raw_content)
}

pub async fn preview_rss(client: &Client, url: &str) -> Result<Vec<RssPreviewItem>> {
    let items = fetch_rss_items(client, url, 3).await?;
    Ok(items
        .into_iter()
        .map(|i| RssPreviewItem {
            title: i.title,
            description: i.description,
            link: i.link,
            image_url: i.image_url,
            pub_date: i.pub_date,
        })
        .collect())
}

pub fn clean_html(input: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    let text = re.replace_all(input, " ");
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    strip_feed_boilerplate(&text)
}

/// RSS feeds often end descriptions with `<a href="...">Read more</a>` — tag stripping leaves the link text.
fn strip_feed_boilerplate(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    static SUFFIXES: &[&str] = &[
        "read more",
        "read more.",
        "continue reading",
        "continue reading.",
        "see more",
        "see more.",
        "full story",
        "full story.",
        "read the full story",
        "read the full story.",
        "view full article",
        "view full article.",
        "читать далее",
        "читать далее.",
        "подробнее",
        "подробнее.",
    ];

    let lower = trimmed.to_ascii_lowercase();
    for suffix in SUFFIXES {
        if lower.ends_with(suffix) {
            let cut = trimmed.len().saturating_sub(suffix.len());
            return trimmed[..cut].trim_end().trim_end_matches('.').trim().to_string();
        }
    }

    trimmed.to_string()
}

fn extract_image_from_entry(entry: &rss::Item) -> Option<String> {
    if let Some(enclosure) = entry.enclosure() {
        let mime = enclosure.mime_type();
        if mime.starts_with("image/") {
            return Some(enclosure.url().to_string());
        }
    }

    if let Some(media) = entry.extensions().get("media") {
        if let Some(content) = media.get("content") {
            for ext in content {
                if let Some(url) = ext.attrs().get("url") {
                    return Some(url.to_string());
                }
            }
        }
        if let Some(thumbnail) = media.get("thumbnail") {
            for ext in thumbnail {
                if let Some(url) = ext.attrs().get("url") {
                    return Some(url.to_string());
                }
            }
        }
    }

    let desc = entry
        .description()
        .or_else(|| entry.content())
        .unwrap_or("");
    extract_img_from_html(desc)
}

fn extract_img_from_html(html: &str) -> Option<String> {
    let re = Regex::new(r#"<img[^>]+src=["']([^"']+)["']"#).ok()?;
    re.captures(html).map(|c| c[1].to_string())
}

pub async fn fetch_og_image(client: &Client, url: &str) -> Option<String> {
    let response = client
        .get(url)
        .header("User-Agent", RSS_USER_AGENT)
        .send()
        .await
        .ok()?;
    let html = response.text().await.ok()?;

    let patterns = [
        r#"<meta[^>]+property=["']og:image["'][^>]+content=["']([^"']+)["']"#,
        r#"<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:image["']"#,
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(caps) = re.captures(&html) {
                return Some(caps[1].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_html_strips_read_more_link_text() {
        let html = r#"It's somehow been seven years. <a href="https://example.com/article">Read more</a>"#;
        let out = clean_html(html);
        assert!(!out.to_ascii_lowercase().contains("read more"));
        assert!(out.contains("seven years"));
    }

    #[test]
    fn clean_html_preserves_normal_text() {
        let html = "Normal description without boilerplate.";
        assert_eq!(clean_html(html), "Normal description without boilerplate.");
    }
}
