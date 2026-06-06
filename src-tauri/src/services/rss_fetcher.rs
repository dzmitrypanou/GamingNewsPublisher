use crate::models::{PresetSource, RssPreviewItem};
use anyhow::{Context, Result};
use regex::Regex;
use reqwest::Client;
use rss::Channel;

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
            url: "https://kotaku.com/rss".to_string(),
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
            url: "https://www.gematsu.com/feed".to_string(),
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
    let response = client
        .get(url)
        .header("User-Agent", "GamingNewsPublisher/0.1")
        .send()
        .await
        .context("Failed to fetch RSS")?;

    let bytes = response.bytes().await?;
    let channel = Channel::read_from(&bytes[..]).context("Failed to parse RSS")?;

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

    Ok(items)
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

fn clean_html(input: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    let text = re.replace_all(input, " ");
    let text = text.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
        .header("User-Agent", "GamingNewsPublisher/0.1")
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
