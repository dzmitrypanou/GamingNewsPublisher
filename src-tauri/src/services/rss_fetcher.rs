use crate::models::{PresetSource, RssPreviewItem};
use anyhow::{Context, Result};
use atom_syndication::Feed as AtomFeed;
use once_cell::sync::Lazy;
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

pub fn site_referer(url: &str) -> Option<String> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(https?://[^/]+)").expect("site referer"));
    RE.captures(url)
        .map(|cap| format!("{}/", &cap[1]))
}

pub fn get_preset_sources() -> Vec<PresetSource> {
    vec![
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
            name: "PC Gamer".to_string(),
            url: "https://www.pcgamer.com/rss/".to_string(),
            category_name: "PC".to_string(),
            group: "General Gaming News".to_string(),
        },
        PresetSource {
            name: "Rock Paper Shotgun".to_string(),
            url: "https://www.rockpapershotgun.com/feed".to_string(),
            category_name: "Инди".to_string(),
            group: "General Gaming News".to_string(),
        },
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
        PresetSource {
            name: "Insider Gaming".to_string(),
            url: "https://insider-gaming.com/feed/".to_string(),
            category_name: "Анонсы".to_string(),
            group: "Leaks & Rumors".to_string(),
        },
        PresetSource {
            name: "RB.RU Cybersport".to_string(),
            url: "https://rb.ru/feeds/tag/cybersport/".to_string(),
            category_name: "Киберспорт".to_string(),
            group: "Cybersport".to_string(),
        },
        PresetSource {
            name: "Ars Technica".to_string(),
            url: "https://arstechnica.com/feed/".to_string(),
            category_name: "Наука".to_string(),
            group: "Science".to_string(),
        },
        PresetSource {
            name: "Quanta Magazine".to_string(),
            url: "https://www.quantamagazine.org/feed/".to_string(),
            category_name: "Наука".to_string(),
            group: "Science".to_string(),
        },
    ]
}

pub struct RssItem {
    pub title: String,
    pub description: String,
    pub link: String,
    pub image_url: Option<String>,
    pub pub_date: Option<String>,
    pub categories: Vec<String>,
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

fn entry_rss_body(entry: &rss::Item) -> String {
    if let Some(content) = entry.content() {
        if !content.trim().is_empty() {
            return content.to_string();
        }
    }

    if let Some(content_ns) = entry.extensions().get("content") {
        if let Some(encoded_exts) = content_ns.get("encoded") {
            for ext in encoded_exts {
                if let Some(value) = ext.value() {
                    if !value.trim().is_empty() {
                        return value.to_string();
                    }
                }
            }
        }
    }

    entry.description().unwrap_or("").to_string()
}

fn parse_rss_channel(channel: &Channel, limit: usize) -> Vec<RssItem> {
    let mut items = Vec::new();
    for entry in channel.items().iter().take(limit) {
        let title = entry.title().unwrap_or("").to_string();
        let description = clean_html(&entry_rss_body(entry));
        let link = entry
            .link()
            .or_else(|| entry.guid().map(|g| g.value()))
            .unwrap_or("")
            .to_string();

        let image_url = extract_image_from_entry(entry);
        let pub_date = entry.pub_date().map(|s| s.to_string());
        let categories = entry
            .categories()
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        if !link.is_empty() {
            items.push(RssItem {
                title,
                description,
                link,
                image_url,
                pub_date,
                categories,
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
        let categories = entry
            .categories()
            .iter()
            .map(|c| c.term().to_string())
            .collect();

        if !link.is_empty() {
            items.push(RssItem {
                title,
                description,
                link,
                image_url,
                pub_date,
                categories,
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

pub fn strip_boilerplate(text: &str) -> String {
    strip_feed_boilerplate(text)
}

fn strip_feed_boilerplate(text: &str) -> String {
    let mut result = text.trim().to_string();
    if result.is_empty() {
        return result;
    }

    for re in BOILERPLATE_TAIL_PATTERNS.iter() {
        result = re.replace(&result, "").into_owned();
        result = result.trim().to_string();
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
        "source",
        "source.",
        "читать далее",
        "читать далее.",
        "подробнее",
        "подробнее.",
        "подробнее в статье",
        "подробнее в статье.",
        "подробности в статье",
        "подробности в статье.",
    ];

    loop {
        let lower = result.to_ascii_lowercase();
        let mut stripped = false;
        for suffix in SUFFIXES {
            if lower.ends_with(suffix) {
                let cut = result.len().saturating_sub(suffix.len());
                result = result[..cut].trim_end().trim_end_matches('.').trim().to_string();
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }

    result
}

pub fn is_boilerplate_paragraph(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.len() > 280 {
        return false;
    }
    BOILERPLATE_PARAGRAPH_PATTERNS
        .iter()
        .any(|re| re.is_match(trimmed))
}

static BOILERPLATE_TAIL_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [

        r"(?is)\s*the post\b.*?\bfirst appeared on\b.*$",

        r"(?is)\s+(?:\S+\s+)?appeared first on\b.*$",
        r"(?is)\s*\boriginally published (?:on|at|in)\b.*$",
        r"(?is)\s*\bas originally published (?:on|at|in)\b.*$",
        r"(?is)\s*\bthis (?:story|article) (?:was )?(?:originally )?published (?:on|at|in)\b.*$",
        r"(?is)\s*\bcontinue reading (?:at|on)\b.*$",
        r"(?is)\s*\bclick here to (?:read|view)\b.*$",
        r"(?is)\s*\bfor more, (?:read|visit|see)\b.*$",
        r"(?is)\s*\bподробнее в статье\.?\s*$",
        r"(?is)\s*\bподробности в статье\.?\s*$",
        r"(?is)\s*\bimage credit\b:.*$",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

static BOILERPLATE_PARAGRAPH_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        r"(?is)^the post\b.*\bfirst appeared on\b",
        r"(?is)^.*\bappeared first on\b",
        r"(?is)^(?:read|view|see) (?:more|full (?:story|article))\.?$",
        r"(?is)^continue reading\.?$",
        r"(?is)^source\.?$",
        r"(?is)^originally published (?:on|at|in)\b",
        r"(?is)^this (?:story|article) (?:was )?(?:originally )?published\b",
        r"(?is)^читать далее\.?$",
        r"(?is)^подробнее\.?$",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

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

    extract_img_from_html(&entry_rss_body(entry))
}

fn extract_img_from_html(html: &str) -> Option<String> {
    let re = Regex::new(r#"<img[^>]+src=["']([^"']+)["']"#).ok()?;
    re.captures(html).map(|c| c[1].to_string())
}

pub async fn fetch_og_image(client: &Client, url: &str) -> Option<String> {
    let mut request = client
        .get(url)
        .header("User-Agent", RSS_USER_AGENT)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        );

    if let Some(referer) = site_referer(url) {
        request = request.header("Referer", referer);
    }

    let response = request.send().await.ok()?;
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

    #[test]
    fn clean_html_strips_appeared_first_on_attribution() {
        let html = r#"See how Taiwan leads the chip industry today. <a href="https://example.com/">appeared first on Example Site</a>."#;
        let out = clean_html(html);
        assert!(out.contains("Taiwan leads"));
        assert!(!out.to_ascii_lowercase().contains("appeared first on"));
        assert!(!out.to_ascii_lowercase().contains("example site"));
    }

    #[test]
    fn clean_html_strips_wordpress_first_appeared_on() {
        let html = "Planarians are fascinating. The post Title first appeared on Quanta Magazine.";
        let out = clean_html(html);
        assert!(out.contains("Planarians"));
        assert!(!out.to_ascii_lowercase().contains("first appeared on"));
    }

    #[test]
    fn site_referer_extracts_origin() {
        assert_eq!(
            site_referer("https://www.example.com/article").as_deref(),
            Some("https://www.example.com/")
        );
        assert_eq!(
            site_referer("https://www.example.com/uploads/test.jpg").as_deref(),
            Some("https://www.example.com/")
        );
    }

    #[test]
    fn ign_feed_extracts_image_from_content_encoded() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
            <channel><item>
                <title>Warhammer</title>
                <description></description>
                <content:encoded><![CDATA[<section class="article-page"><img src="https://assets-prd.ignimgs.com/2026/06/05/dark-heresy-1780691362412.jpg"/>]]></content:encoded>
            </item></channel></rss>"#;
        let channel = Channel::read_from(xml.as_bytes()).expect("parse feed");
        let url = extract_image_from_entry(&channel.items()[0]);
        assert_eq!(
            url.as_deref(),
            Some("https://assets-prd.ignimgs.com/2026/06/05/dark-heresy-1780691362412.jpg")
        );
    }

    #[test]
    fn feed_extracts_enclosure_image() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <rss version="2.0"><channel><item>
                <title>Sample article</title>
                <link>https://example.com/article/</link>
                <enclosure url="https://example.com/uploads/test.jpg" type="image/jpeg"/>
            </item></channel></rss>"#;
        let channel = Channel::read_from(xml.as_bytes()).expect("parse feed");
        let url = extract_image_from_entry(&channel.items()[0]);
        assert_eq!(
            url.as_deref(),
            Some("https://example.com/uploads/test.jpg")
        );
    }

    #[test]
    fn feed_extracts_media_thumbnail() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
            <channel><item>
                <title>Test</title>
                <media:thumbnail url="https://example.com/uploads/test.jpg" />
            </item></channel></rss>"#;
        let channel = Channel::read_from(xml.as_bytes()).expect("parse feed");
        let url = extract_image_from_entry(&channel.items()[0]);
        assert_eq!(
            url.as_deref(),
            Some("https://example.com/uploads/test.jpg")
        );
    }

    #[test]
    fn quanta_feed_extracts_media_content_images() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
            <channel><item>
                <title>WebP item</title>
                <media:content url="https://example.com/image.webp" type="image/jpg"/>
            </item><item>
                <title>JPG item</title>
                <media:content url="https://example.com/image.jpg" type="image/jpg"/>
            </item></channel></rss>"#;
        let channel = Channel::read_from(xml.as_bytes()).expect("parse feed");
        let webp = extract_image_from_entry(&channel.items()[0]);
        assert_eq!(webp.as_deref(), Some("https://example.com/image.webp"));
        let jpg = extract_image_from_entry(&channel.items()[1]);
        assert_eq!(jpg.as_deref(), Some("https://example.com/image.jpg"));
    }

    #[test]
    fn is_boilerplate_paragraph_detects_attribution_line() {
        assert!(is_boilerplate_paragraph("a appeared first on Example Site ."));
        assert!(is_boilerplate_paragraph(
            "The post My Title first appeared on Quanta Magazine."
        ));
        assert!(!is_boilerplate_paragraph(
            "While there's never any shortage of games releasing on Steam any given day."
        ));
    }
}
