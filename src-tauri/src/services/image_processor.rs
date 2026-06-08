use crate::models::AppSettings;
use crate::services::data_dir;
use crate::services::watermark::{self, WatermarkConfig};

use crate::services::image_loader::LOCAL_IMAGE_PREFIX;

use crate::services::rss_fetcher;

use anyhow::{Context, Result};

use image::codecs::jpeg::JpegEncoder;

use image::{DynamicImage, GenericImageView};

use regex::Regex;

use reqwest::Client;

use sha2::{Digest, Sha256};

use std::collections::HashSet;

use std::path::Path;

pub const DEFAULT_POST_IMAGE_WIDTH: u32 = 1280;

pub const DEFAULT_POST_IMAGE_HEIGHT: u32 = 720;

const MIN_POST_IMAGE_WIDTH: u32 = 320;

const MAX_POST_IMAGE_WIDTH: u32 = 4096;

const MIN_POST_IMAGE_HEIGHT: u32 = 180;

const MAX_POST_IMAGE_HEIGHT: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostImageSize {
    pub width: u32,
    pub height: u32,
}

impl Default for PostImageSize {
    fn default() -> Self {
        Self {
            width: DEFAULT_POST_IMAGE_WIDTH,
            height: DEFAULT_POST_IMAGE_HEIGHT,
        }
    }
}

impl PostImageSize {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            width: settings
                .post_image_width
                .clamp(MIN_POST_IMAGE_WIDTH, MAX_POST_IMAGE_WIDTH),
            height: settings
                .post_image_height
                .clamp(MIN_POST_IMAGE_HEIGHT, MAX_POST_IMAGE_HEIGHT),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostImageOptions {
    pub size: PostImageSize,
    pub watermark: WatermarkConfig,
}

impl PostImageOptions {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            size: PostImageSize::from_settings(settings),
            watermark: WatermarkConfig::from_settings(settings),
        }
    }
}

impl Default for PostImageOptions {
    fn default() -> Self {
        Self {
            size: PostImageSize::default(),
            watermark: WatermarkConfig::from_settings(&AppSettings::default()),
        }
    }
}

// Remove IGN watermark badge on the left edge.

const IGN_LEFT_CROP_FRACTION: f32 = 0.30;

pub fn is_ign_source(source_url: &str, article_url: &str, image_url: Option<&str>) -> bool {

    let source = source_url.to_ascii_lowercase();

    let article = article_url.to_ascii_lowercase();

    if source.contains("ign.com")

        || source.contains("feedburner.com/ign")

        || article.contains("ign.com")

    {

        return true;

    }

    image_url

        .map(|url| url.to_ascii_lowercase().contains("ignimgs.com"))

        .unwrap_or(false)

}

pub fn remote_image_hint(current: Option<&str>) -> Option<&str> {
    current.filter(|url| url.starts_with("http://") || url.starts_with("https://"))
}

pub fn guess_feed_source_url(
    sources: &[crate::models::Source],
    article_url: &str,
    category_id: Option<i64>,
) -> String {
    if let Some(cid) = category_id {
        if let Some(source) = sources.iter().find(|s| s.category_id == Some(cid)) {
            return source.url.clone();
        }
    }

    let article = article_url.to_ascii_lowercase();
    if let Some(source) = sources.iter().find(|s| {
        s.url
            .to_ascii_lowercase()
            .split('/')
            .nth(2)
            .map(|host| !host.is_empty() && article.contains(host))
            .unwrap_or(false)
    }) {
        return source.url.clone();
    }

    String::new()
}

pub async fn resolve_post_image(

    client: &Client,

    data_dir: &Path,

    article_url: &str,

    source_url: &str,

    title: &str,

    rss_image: Option<&str>,

    options: PostImageOptions,

) -> Option<String> {

    let _ = title;

    if is_ign_source(source_url, article_url, rss_image) {

        return match resolve_ign_image(client, data_dir, article_url, rss_image, options).await {

            Ok(Some(url)) => Some(url),

            Ok(None) => rss_image.and_then(|url| fallback_remote_url(url)),

            Err(e) => {

                eprintln!("IGN image resolve {}: {}", article_url, e);

                rss_image.and_then(|url| fallback_remote_url(url))

            }

        };

    }

    let mut image_url = rss_image.map(String::from);

    if image_url.is_none() {

        image_url = rss_fetcher::fetch_og_image(client, article_url).await;

    }

    let Some(url) = image_url else {

        return None;

    };

    match download_and_save_post_image(
        client,
        data_dir,
        &url,
        article_url,
        false,
        options.clone(),
    )
    .await
    {
        Ok(local_ref) => Some(local_ref),
        Err(e) => {
            eprintln!("Image normalize {}: {}", url, e);
            // Do not fall back to og:image here — it would require fetching the article page.
            // Just keep the original URL (rss thumbnail or previously fetched og:image).
            fallback_remote_url(&url)
        }
    }

}

async fn resolve_ign_image(

    client: &Client,

    data_dir: &Path,

    article_url: &str,

    rss_image: Option<&str>,

    options: PostImageOptions,

) -> Result<Option<String>> {

    let candidates =
        fetch_ign_image_candidates(client, article_url, options.size.width).await?;

    let image_url = pick_ign_image_url(&candidates, rss_image);

    let Some(image_url) = image_url else {

        return Ok(None);

    };

    let local_ref = download_and_save_post_image(
        client,
        data_dir,
        &image_url,
        article_url,
        ign_needs_left_crop(&image_url),
        options,
    )
    .await?;
    Ok(Some(local_ref))

}

async fn download_and_save_post_image(
    client: &Client,
    data_dir: &Path,
    image_url: &str,
    article_url: &str,
    is_ign: bool,
    options: PostImageOptions,
) -> Result<String> {
    let referer = rss_fetcher::site_referer(article_url)
        .or_else(|| rss_fetcher::site_referer(image_url));
    let bytes = download_image_bytes(client, image_url, referer.as_deref()).await?;
    let processed = match process_post_image_bytes(&bytes, is_ign, options, data_dir) {
        Ok(processed) => processed,
        Err(e) => {
            eprintln!("Image process {}: {}", image_url, e);
            return Ok(fallback_remote_url(image_url).unwrap_or_else(|| image_url.to_string()));
        }
    };
    save_local_image(data_dir, &processed, image_url)
}

fn pick_ign_image_url(candidates: &[String], rss_image: Option<&str>) -> Option<String> {
    let heroes: Vec<String> = candidates
        .iter()
        .filter(|url| is_ign_article_image(url))
        .cloned()
        .collect();

    if heroes.is_empty() {
        return rss_image.map(String::from);
    }

    let rss_base = rss_image.and_then(normalize_ign_base);
    let primary_base = rss_base
        .clone()
        .or_else(|| normalize_ign_base(&heroes[0]));

    if let Some(base) = primary_base {
        let same_base: Vec<&String> = heroes
            .iter()
            .filter(|url| normalize_ign_base(url).as_deref() == Some(base.as_str()))
            .collect();

        if let Some(best) = same_base
            .iter()
            .filter(|url| url.contains("canvas="))
            .max_by_key(|url| ign_download_width(url))
        {
            return Some((*best).clone());
        }

        if rss_base.is_some() {
            if let Some(alt) = heroes
                .iter()
                .find(|url| normalize_ign_base(url).as_deref() != Some(base.as_str()))
            {
                return Some(alt.clone());
            }
        }

        if let Some(first) = same_base.first() {
            return Some((*first).clone());
        }
    }

    heroes.first().cloned()
}

fn is_ign_article_image(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.contains("/avatars/")
        || lower.contains("/registration/")
        || lower.contains("/kraken/")
    {
        return false;
    }

    lower.contains("ignimgs.com/") && lower.contains("/20")
}

fn ign_download_width(url: &str) -> u32 {
    url.split('&')
        .chain(url.split('?'))
        .find_map(|part| part.strip_prefix("width="))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn ign_needs_left_crop(image_url: &str) -> bool {
    !image_url.contains("canvas=")
}

fn fallback_remote_url(url: &str) -> Option<String> {

    if url.starts_with("local:") {

        None

    } else {

        Some(url.to_string())

    }

}

pub fn crop_ign_left_strip(img: &DynamicImage) -> DynamicImage {

    let (w, h) = img.dimensions();

    let left = (w as f32 * IGN_LEFT_CROP_FRACTION)
        .round()
        .clamp(0.0, (w.saturating_sub(1)) as f32) as u32;

    let crop_w = w.saturating_sub(left).max(1);

    img.crop_imm(left, 0, crop_w, h)

}

pub fn fit_cover_to_post_template(img: &DynamicImage, size: PostImageSize) -> DynamicImage {

    let target_aspect = size.width as f32 / size.height as f32;

    let (w, h) = img.dimensions();

    let src_aspect = w as f32 / h as f32;

    let cropped = if (src_aspect - target_aspect).abs() < 0.001 {

        img.clone()

    } else if src_aspect > target_aspect {

        let new_w = (h as f32 * target_aspect).round().max(1.0) as u32;

        let x = (w.saturating_sub(new_w)) / 2;

        img.crop_imm(x, 0, new_w.min(w), h)

    } else {

        let new_h = (w as f32 / target_aspect).round().max(1.0) as u32;

        let y = (h.saturating_sub(new_h)) / 2;

        img.crop_imm(0, y, w, new_h.min(h))

    };

    cropped.resize_exact(

        size.width,

        size.height,

        image::imageops::FilterType::Triangle,

    )

}

pub fn process_post_image_bytes(
    bytes: &[u8],
    is_ign: bool,
    options: PostImageOptions,
    data_dir: &Path,
) -> Result<Vec<u8>> {

    let img = image::load_from_memory(bytes).context("Decode image")?;

    let framed = if is_ign {

        crop_ign_left_strip(&img)

    } else {

        img

    };

    let fitted = fit_cover_to_post_template(&framed, options.size);
    let with_watermark = watermark::apply_watermark_from_settings(
        &fitted,
        data_dir,
        options.size,
        &options.watermark,
    )?;

    encode_jpeg(&with_watermark)

}

pub async fn fetch_ign_image_candidates(
    client: &Client,
    article_url: &str,
    download_width: u32,
) -> Result<Vec<String>> {

    let response = client
        .get(article_url)
        .header("User-Agent", rss_fetcher::user_agent())
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        );

    let mut request = response;
    if let Some(referer) = rss_fetcher::site_referer(article_url) {
        request = request.header("Referer", referer);
    }

    let response = request
        .send()
        .await
        .context("IGN article fetch failed")?;

    if !response.status().is_success() {

        anyhow::bail!("IGN article HTTP {}", response.status());

    }

    let html = response.text().await?;

    Ok(extract_ign_image_urls(&html, download_width))

}

fn extract_ign_image_urls(html: &str, download_width: u32) -> Vec<String> {

    let mut seen = HashSet::new();

    let mut urls = Vec::new();

    let patterns = [

        r#"property=["']og:image["'][^>]+content=["']([^"']+ignimgs\.com[^"']+)["']"#,

        r#"content=["']([^"']+ignimgs\.com[^"']+)["'][^>]+property=["']og:image["']"#,

        r#"name=["']thumbnail["'][^>]+content=["']([^"']+ignimgs\.com[^"']+)["']"#,

        r#"content=["']([^"']+ignimgs\.com[^"']+)["'][^>]+name=["']thumbnail["']"#,

        r#"https://assets-prd\.ignimgs\.com/[a-zA-Z0-9_./%-]+\.(?:jpe?g|png|webp)(?:\?[^"'\s<>]*)?"#,

        r#"https://assets\.ignimgs\.com/[a-zA-Z0-9_./%-]+\.(?:jpe?g|png|webp)(?:\?[^"'\s<>]*)?"#,

    ];

    for pattern in patterns {

        if let Ok(re) = Regex::new(pattern) {

            for caps in re.captures_iter(html) {

                let raw = caps

                    .get(1)

                    .map(|m| m.as_str())

                    .unwrap_or_else(|| caps.get(0).unwrap().as_str());

                if let Some(normalized) = normalize_ign_image_url(raw, download_width) {
                    if is_ign_article_image(&normalized) && seen.insert(normalized.clone()) {
                        urls.push(normalized);
                    }
                }

            }

        }

    }

    urls

}

fn normalize_ign_image_url(url: &str, download_width: u32) -> Option<String> {

    let mut cleaned = url

        .trim()

        .trim_end_matches(&['"', '\'', ';'][..])

        .replace("&amp;", "&");

    if cleaned.contains("/registration/") {

        return None;

    }

    if let Some(base) = cleaned.split('?').next() {

        if base.ends_with(".png")

            || base.ends_with(".webp")

            || base.ends_with(".jpg")

            || base.ends_with(".jpeg")

        {

            cleaned = format!(
                "{base}?width={download_width}&format=jpg&auto=webp&quality=85"
            );

        }

    }

    if cleaned.contains("ignimgs.com") {

        Some(cleaned)

    } else {

        None

    }

}

pub fn pick_best_candidate(candidates: &[String], rss_image: Option<&str>) -> Option<String> {

    if candidates.is_empty() {

        return None;

    }

    let rss_base = rss_image.and_then(normalize_ign_base);

    if let Some(base) = rss_base {

        for candidate in candidates {

            if normalize_ign_base(candidate).as_deref() != Some(base.as_str()) {

                return Some(candidate.clone());

            }

        }

    }

    candidates.first().cloned()

}

fn normalize_ign_base(url: &str) -> Option<String> {

    let base = url.split('?').next()?.trim();

    if base.is_empty() {

        None

    } else {

        Some(base.to_string())

    }

}

pub async fn download_image_bytes(
    client: &Client,
    url: &str,
    referer: Option<&str>,
) -> Result<Vec<u8>> {
    let mut request = client
        .get(url)
        .header("User-Agent", rss_fetcher::user_agent())
        .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8");

    if let Some(referer) = referer {
        request = request.header("Referer", referer);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("Download image {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("Image HTTP {} for {}", response.status(), url);
    }

    Ok(response.bytes().await?.to_vec())
}

fn encode_jpeg(img: &DynamicImage) -> Result<Vec<u8>> {

    let rgb = img.to_rgb8();

    let mut out = Vec::new();

    let mut encoder = JpegEncoder::new_with_quality(&mut out, 85);

    encoder

        .encode(

            rgb.as_raw(),

            rgb.width(),

            rgb.height(),

            image::ColorType::Rgb8.into(),

        )

        .context("Encode JPEG")?;

    Ok(out)

}

pub fn save_local_image(data_dir: &Path, bytes: &[u8], source_url: &str) -> Result<String> {

    let images_dir = data_dir::images_dir(data_dir);

    let hash = Sha256::digest(bytes);

    let filename = format!("{:x}.jpg", hash);

    let path = images_dir.join(&filename);

    if !path.exists() {

        std::fs::write(&path, bytes).with_context(|| format!("Write {}", path.display()))?;

    }

    let _ = source_url;

    Ok(format!("{LOCAL_IMAGE_PREFIX}images/{filename}"))

}

#[cfg(test)]

mod tests {

    use super::*;

    use image::Rgb;

    use image::{ImageBuffer, RgbImage};

    fn make_canvas(width: u32, height: u32) -> RgbImage {

        let mut img = RgbImage::new(width, height);

        for y in 0..height {

            for x in 0..width {

                let r = ((x * 40 + y * 15) % 200 + 30) as u8;

                let g = ((x * 25 + y * 35) % 180 + 20) as u8;

                let b = ((x * 55 + y * 10) % 190 + 25) as u8;

                img.put_pixel(x, y, Rgb([r, g, b]));

            }

        }

        img

    }

    #[test]
    fn picks_alternate_hero_when_rss_thumb_differs() {
        let rss = Some("https://assets-prd.ignimgs.com/2026/06/a.jpeg");
        let candidates = vec![
            "https://assets-prd.ignimgs.com/2026/06/a.jpeg?width=1280".to_string(),
            "https://assets-prd.ignimgs.com/2026/06/b.jpeg?width=1280".to_string(),
        ];
        let picked = pick_ign_image_url(&candidates, rss).unwrap();
        assert!(picked.contains("/b.jpeg"));
    }

    #[test]
    fn ign_picks_hero_not_author_avatar() {
        let rss = Some("https://assets-prd.ignimgs.com/2026/06/05/dark-heresy-1780691362412.jpg");
        let candidates = vec![
            "https://assets-prd.ignimgs.com/2026/06/05/dark-heresy-1780691362412.jpg?width=1280&format=jpg&auto=webp&quality=80".to_string(),
            "https://assets-prd.ignimgs.com/avatars/640f55c4f3866b000174a9c6/RD5zbE5b_400x400-1679594933868.jpg?crop=1%3A1&width=21&format=jpg&auto=webp&quality=80".to_string(),
            "https://assets-prd.ignimgs.com/2026/06/05/dark-heresy-1780691362412.jpg?width=1280&canvas=1280%2C720&format=jpg&auto=webp&quality=80".to_string(),
        ];
        let picked = pick_ign_image_url(&candidates, rss).unwrap();
        assert!(picked.contains("dark-heresy"));
        assert!(!picked.contains("/avatars/"));
        assert!(picked.contains("canvas=1280"));
    }

    #[test]
    fn ign_article_image_filter_drops_avatars_and_logos() {
        assert!(is_ign_article_image(
            "https://assets-prd.ignimgs.com/2026/06/05/dark-heresy-1780691362412.jpg"
        ));
        assert!(!is_ign_article_image(
            "https://assets-prd.ignimgs.com/avatars/640f55c4f3866b000174a9c6/RD5zbE5b_400x400.jpg"
        ));
        assert!(!is_ign_article_image(
            "https://assets1.ignimgs.com/kraken/ign30-logo.png?width=96"
        ));
    }

    #[test]
    fn picks_alternate_candidate() {

        let rss = Some("https://assets-prd.ignimgs.com/2026/06/a.jpeg");

        let candidates = vec![

            "https://assets-prd.ignimgs.com/2026/06/a.jpeg?width=1280".to_string(),

            "https://assets-prd.ignimgs.com/2026/06/b.jpeg?width=1280".to_string(),

        ];

        let picked = pick_best_candidate(&candidates, rss).unwrap();

        assert!(picked.contains("/b.jpeg"));

    }

    #[test]

    fn ign_left_crop_removes_thirty_percent_width() {

        let img = DynamicImage::ImageRgb8(make_canvas(480, 270));

        let cropped = crop_ign_left_strip(&img);

        assert_eq!(cropped.width(), 336);

        assert_eq!(cropped.height(), 270);

    }

    #[test]

    fn fit_cover_outputs_1280x720() {

        let img = DynamicImage::ImageRgb8(make_canvas(1920, 1080));

        let size = PostImageSize::default();

        let fitted = fit_cover_to_post_template(&img, size);

        assert_eq!(fitted.width(), DEFAULT_POST_IMAGE_WIDTH);

        assert_eq!(fitted.height(), DEFAULT_POST_IMAGE_HEIGHT);

    }

    #[test]

    fn process_ign_pipeline_outputs_template() {

        let mut img: RgbImage = ImageBuffer::new(480, 270);

        for pixel in img.pixels_mut() {

            *pixel = Rgb([0, 180, 255]);

        }

        for y in 0..50 {

            for x in 0..120 {

                img.put_pixel(x, y, Rgb([255, 220, 0]));

            }

        }

        let bytes = {

            let dynamic = DynamicImage::ImageRgb8(img);

            let processed = process_post_image_bytes(

                &encode_jpeg(&dynamic).unwrap(),

                true,

                PostImageOptions::default(),

                std::path::Path::new("."),

            )

            .unwrap();

            processed

        };

        let decoded = image::load_from_memory(&bytes).unwrap();

        assert_eq!(decoded.width(), DEFAULT_POST_IMAGE_WIDTH);

        assert_eq!(decoded.height(), DEFAULT_POST_IMAGE_HEIGHT);

    }

    #[test]

    fn process_standard_pipeline_outputs_template() {

        let img = DynamicImage::ImageRgb8(make_canvas(1600, 900));

        let bytes = process_post_image_bytes(
            &encode_jpeg(&img).unwrap(),
            false,
            PostImageOptions::default(),
            std::path::Path::new("."),
        )
        .unwrap();

        let decoded = image::load_from_memory(&bytes).unwrap();

        assert_eq!(decoded.width(), DEFAULT_POST_IMAGE_WIDTH);

        assert_eq!(decoded.height(), DEFAULT_POST_IMAGE_HEIGHT);

    }

    #[test]

    fn real_ign_motu_image_fits_template() {

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-ign.jpg");

        if !path.exists() {

            return;

        }

        let bytes = std::fs::read(&path).expect("read test image");

        let processed = process_post_image_bytes(
            &bytes,
            true,
            PostImageOptions::default(),
            std::path::Path::new("."),
        )
        .expect("process");

        let decoded = image::load_from_memory(&processed).expect("decode");

        assert_eq!(decoded.width(), DEFAULT_POST_IMAGE_WIDTH);

        assert_eq!(decoded.height(), DEFAULT_POST_IMAGE_HEIGHT);

    }

}

