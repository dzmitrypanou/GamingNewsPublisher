use crate::models::{ApiTestResult, AppSettings};
use crate::services::image_loader;
use crate::services::post_text;
use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde_json::Value;
use std::path::Path;

pub fn normalize_group_id(raw: &str) -> Result<String> {
    let mut value = raw.trim();
    if value.is_empty() {
        bail!("ID группы не указан");
    }

    if let Some((path, _)) = value.split_once('?') {
        value = path;
    }

    if value.contains("vk.com/") {
        value = value
            .rsplit('/')
            .next()
            .context("Не удалось извлечь ID группы из ссылки VK")?;
    }

    value = value.trim().trim_start_matches('-');

    let lower = value.to_ascii_lowercase();
    for prefix in ["club", "public", "event"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            value = &value[prefix.len()..];
            if rest.is_empty() {
                bail!("Некорректный ID группы: укажите число, например 123456789");
            }
            break;
        }
    }

    value = value.trim();
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
        bail!(
            "Некорректный ID группы «{}»: укажите число без club/public и без минуса",
            raw.trim()
        );
    }

    Ok(value.to_string())
}

fn normalize_token(raw: &str) -> Result<String> {
    let token = raw.trim();
    if token.is_empty() {
        bail!("Токен VK не указан. Сохраните настройки после ввода токена.");
    }
    Ok(token.to_string())
}

fn group_credentials(settings: &AppSettings) -> Result<(String, String)> {
    Ok((
        normalize_token(&settings.vk_token)?,
        normalize_group_id(&settings.vk_group_id)?,
    ))
}

fn photo_upload_token(settings: &AppSettings, community_token: &str) -> String {
    let user = settings.vk_user_token.trim();
    if user.is_empty() {
        community_token.to_string()
    } else {
        user.to_string()
    }
}

fn has_user_photo_token(settings: &AppSettings) -> bool {
    !settings.vk_user_token.trim().is_empty()
}

fn photo_upload_failure_message(settings: &AppSettings, err: &anyhow::Error) -> String {
    let detail = err.to_string();
    if has_user_photo_token(settings) {
        return format!(
            "Не удалось загрузить фото в VK: {detail}. \
             Проверьте права user token (wall, photos, offline) и что аккаунт — админ/редактор группы."
        );
    }
    if is_group_auth_photo_error(&detail) || is_scope_error(&detail) {
        return format!(
            "Не удалось загрузить фото в VK: {detail}. \
             Ключ сообщества не может загружать фото (ограничение VK API). \
             Укажите user token в настройках — получите на vkhost.github.io с правами «Стена», «Фотографии», «Offline»."
        );
    }
    format!(
        "Не удалось загрузить фото в VK: {detail}. \
         Для публикации с картинкой нужен user token (vkhost.github.io: wall + photos + offline)."
    )
}

fn json_to_api_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

async fn vk_method(
    client: &Client,
    method: &str,
    mut params: Vec<(&str, String)>,
) -> Result<Value> {
    params.push(("v", "5.199".to_string()));

    let resp: Value = client
        .post(format!("https://api.vk.com/method/{method}"))
        .form(&params)
        .send()
        .await
        .with_context(|| format!("VK {method}: сеть недоступна"))?
        .json()
        .await
        .with_context(|| format!("VK {method}: некорректный ответ"))?;

    if let Some(err) = resp.get("error") {
        let msg = err["error_msg"]
            .as_str()
            .unwrap_or("VK API error");
        bail!("{}", msg);
    }

    Ok(resp)
}

fn group_from_response(resp: &Value) -> Result<&Value> {
    if let Some(group) = resp["response"]["groups"].get(0) {
        return Ok(group);
    }
    if let Some(group) = resp["response"].get(0) {
        return Ok(group);
    }
    bail!("Сообщество не найдено в ответе VK");
}

async fn get_group_info(client: &Client, token: &str, group_id: &str) -> Result<(String, bool)> {
    let resp = vk_method(
        client,
        "groups.getById",
        vec![
            ("group_id", group_id.to_string()),
            ("fields", "can_post".to_string()),
            ("access_token", token.to_string()),
        ],
    )
    .await?;

    let group = group_from_response(&resp)?;
    let name = group["name"]
        .as_str()
        .unwrap_or("Группа")
        .to_string();
    let can_post = group["can_post"].as_i64().unwrap_or(1) == 1;
    Ok((name, can_post))
}

async fn verify_wall_upload(client: &Client, token: &str, group_id: &str) -> Result<()> {
    let resp = vk_method(
        client,
        "photos.getWallUploadServer",
        vec![
            ("group_id", group_id.to_string()),
            ("access_token", token.to_string()),
        ],
    )
    .await?;

    if resp["response"]["upload_url"].as_str().is_none() {
        bail!("VK не вернул upload_url для фото");
    }

    Ok(())
}

fn is_group_auth_photo_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("group auth") || lower.contains("unavailable with group")
}

fn is_scope_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("current scopes") || lower.contains("no access to call")
}

fn link_attachment(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url.to_string())
    } else {
        None
    }
}

/// VK 2025: a link in `attachments` requires a photo (`link_photo_sizing_rule`).
fn build_attachments(photo: Option<String>, link_url: Option<&str>) -> Option<String> {
    match (photo, link_attachment(link_url)) {
        (Some(photo), Some(link)) => Some(format!("{photo},{link}")),
        (Some(photo), None) => Some(photo),
        (None, Some(_)) => None,
        (None, None) => None,
    }
}

fn append_link_to_message(message: &str, link_url: Option<&str>) -> String {
    let Some(url) = link_attachment(link_url) else {
        return message.to_string();
    };
    if post_text::contains_url(message) {
        return message.to_string();
    }
    let msg = message.trim();
    if msg.is_empty() {
        url
    } else {
        format!("{msg}\n\n{url}")
    }
}

const PERM_PHOTOS: u64 = 4;
const PERM_WALL: u64 = 8192;

async fn get_user_token_permissions(client: &Client, token: &str) -> Result<u64> {
    let resp = vk_method(
        client,
        "account.getAppPermissions",
        vec![("access_token", token.to_string())],
    )
    .await?;
    Ok(resp["response"].as_u64().unwrap_or(0))
}

fn format_missing_photo_scopes(mask: u64) -> String {
    let mut missing = Vec::new();
    if mask & PERM_PHOTOS == 0 {
        missing.push("photos");
    }
    if mask & PERM_WALL == 0 {
        missing.push("wall");
    }
    if missing.is_empty() {
        "неизвестные права".to_string()
    } else {
        missing.join(", ")
    }
}

pub async fn test_connection(client: &Client, settings: &AppSettings) -> ApiTestResult {
    let (community_token, group_id) = match group_credentials(settings) {
        Ok(v) => v,
        Err(e) => {
            return ApiTestResult {
                success: false,
                message: e.to_string(),
            };
        }
    };

    match get_group_info(client, &community_token, &group_id).await {
        Ok((name, can_post)) => {
            if !can_post {
                return ApiTestResult {
                    success: false,
                    message: format!(
                        "Группа «{name}» найдена, но ключ сообщества не может публиковать на стене. \
                         Выдайте ключу права wall, photos и groups."
                    ),
                };
            }

            let upload_token = photo_upload_token(settings, &community_token);
            match verify_wall_upload(client, &upload_token, &group_id).await {
                Ok(()) if has_user_photo_token(settings) => ApiTestResult {
                    success: true,
                    message: format!(
                        "Подключено: {name}. Публикация с фото доступна (user token настроен)."
                    ),
                },
                Ok(()) => ApiTestResult {
                    success: true,
                    message: format!("Подключено: {name}. Публикация с фото доступна."),
                },
                Err(e) if has_user_photo_token(settings) => {
                    let scope_hint = if is_scope_error(&e.to_string()) {
                        match get_user_token_permissions(client, &upload_token).await {
                            Ok(mask) => format!(
                                " Не хватает прав: {}. На vkhost.github.io отметьте «Фотографии» и «Стена».",
                                format_missing_photo_scopes(mask)
                            ),
                            Err(_) => String::new(),
                        }
                    } else {
                        String::new()
                    };
                    ApiTestResult {
                        success: false,
                        message: format!(
                            "Группа «{name}» найдена, но upload фото недоступен: {e}.{scope_hint} \
                             Укажите user token с правами wall и photos."
                        ),
                    }
                }
                Err(e) if is_group_auth_photo_error(&e.to_string()) => ApiTestResult {
                    success: false,
                    message: format!(
                        "Группа «{name}» найдена, но без user token фото не загрузить. \
                         Ключ сообщества не поддерживает upload (ограничение VK). \
                         Получите user token на vkhost.github.io (wall + photos + offline) \
                         и вставьте в поле «User token»."
                    ),
                },
                Err(e) => ApiTestResult {
                    success: false,
                    message: format!(
                        "Группа «{name}» найдена, но загрузка фото недоступна: {e}. \
                         Проверьте права photos и wall у токена."
                    ),
                },
            }
        }
        Err(e) => ApiTestResult {
            success: false,
            message: format!("Ошибка: {e}"),
        },
    }
}

pub async fn publish_post(
    client: &Client,
    settings: &AppSettings,
    message: &str,
    image_url: Option<&str>,
    link_url: Option<&str>,
    data_dir: Option<&Path>,
) -> Result<String> {
    let (community_token, group_id) = group_credentials(settings)?;
    let upload_token = photo_upload_token(settings, &community_token);
    let owner_id = format!("-{group_id}");

    let photo_attachment = if let Some(img_url) = image_url {
        upload_photo(client, &upload_token, &group_id, img_url, data_dir)
            .await
            .map(Some)
            .map_err(|e| anyhow::anyhow!(photo_upload_failure_message(settings, &e)))?
    } else {
        None
    };

    let attachment = build_attachments(photo_attachment, link_url);
    let wall_message = if attachment.is_none() {
        append_link_to_message(message, link_url)
    } else {
        message.to_string()
    };

    let mut params = vec![
        ("owner_id", owner_id),
        ("from_group", "1".to_string()),
        ("message", wall_message),
        ("access_token", community_token),
    ];

    if let Some(att) = attachment {
        params.push(("attachments", att));
    }

    let resp = vk_method(client, "wall.post", params).await?;

    let post_id = resp["response"]["post_id"]
        .as_i64()
        .context("No post_id in response")?;
    Ok(post_id.to_string())
}

pub async fn delete_post(
    client: &Client,
    settings: &AppSettings,
    post_id: &str,
) -> Result<()> {
    let (token, group_id) = group_credentials(settings)?;
    let owner_id = format!("-{group_id}");

    vk_method(
        client,
        "wall.delete",
        vec![
            ("owner_id", owner_id),
            ("post_id", post_id.to_string()),
            ("access_token", token),
        ],
    )
    .await?;

    Ok(())
}

async fn upload_photo(
    client: &Client,
    token: &str,
    group_id: &str,
    image_url: &str,
    data_dir: Option<&Path>,
) -> Result<String> {
    let server_resp = vk_method(
        client,
        "photos.getWallUploadServer",
        vec![
            ("group_id", group_id.to_string()),
            ("access_token", token.to_string()),
        ],
    )
    .await?;

    let upload_url = server_resp["response"]["upload_url"]
        .as_str()
        .context("No upload_url")?;

    let img_bytes = if let Some(dir) = data_dir {
        image_loader::load_image_bytes(client, dir, image_url).await?
    } else {
        client
            .get(image_url)
            .header("User-Agent", "Mozilla/5.0 GamingNewsPublisher/0.1")
            .send()
            .await
            .context("Image download failed")?
            .error_for_status()
            .context("Image HTTP error")?
            .bytes()
            .await
            .context("Image body read failed")?
            .to_vec()
    };

    let form = reqwest::multipart::Form::new().part(
        "photo",
        reqwest::multipart::Part::bytes(img_bytes)
            .file_name("photo.jpg")
            .mime_str("image/jpeg")?,
    );

    let upload_resp: Value = client
        .post(upload_url)
        .multipart(form)
        .send()
        .await
        .context("VK photo upload failed")?
        .json()
        .await
        .context("VK photo upload response invalid")?;

    let photo = upload_resp["photo"].as_str().unwrap_or("");
    let server = json_to_api_string(&upload_resp["server"]);
    let hash = upload_resp["hash"].as_str().unwrap_or("");

    if photo.is_empty() || server.is_empty() || hash.is_empty() {
        bail!("VK upload server returned incomplete photo payload");
    }

    let save_resp = vk_method(
        client,
        "photos.saveWallPhoto",
        vec![
            ("group_id", group_id.to_string()),
            ("photo", photo.to_string()),
            ("server", server),
            ("hash", hash.to_string()),
            ("access_token", token.to_string()),
        ],
    )
    .await?;

    let photo_obj = &save_resp["response"][0];
    let photo_id = photo_obj["id"].as_i64().context("No photo id")?;
    let photo_owner = photo_obj["owner_id"].as_i64().context("No photo owner")?;

    Ok(format!("photo{photo_owner}_{photo_id}"))
}

pub fn format_message(title: &str, text: &str, hashtags: &str) -> String {
    let title = title.trim();
    let bold_title = if title.is_empty() {
        String::new()
    } else {
        format!("*{}*", title.replace('*', ""))
    };

    let mut parts = Vec::new();
    if !bold_title.is_empty() {
        parts.push(bold_title);
    }
    if !text.trim().is_empty() {
        if !parts.is_empty() {
            parts.push(String::new());
        }
        parts.push(text.to_string());
    }
    if !hashtags.trim().is_empty() {
        parts.push(String::new());
        parts.push(hashtags.to_string());
    }

    let msg = parts.join("\n");
    if msg.chars().count() > 4096 {
        msg.chars().take(4093).collect::<String>() + "..."
    } else {
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_numeric_group_id() {
        assert_eq!(normalize_group_id("123456789").unwrap(), "123456789");
        assert_eq!(normalize_group_id("-123456789").unwrap(), "123456789");
    }

    #[test]
    fn normalizes_club_prefix_and_url() {
        assert_eq!(normalize_group_id("club225364560").unwrap(), "225364560");
        assert_eq!(
            normalize_group_id("https://vk.com/club225364560").unwrap(),
            "225364560"
        );
    }

    #[test]
    fn rejects_invalid_group_id() {
        assert!(normalize_group_id("").is_err());
        assert!(normalize_group_id("club").is_err());
        assert!(normalize_group_id("not-a-number").is_err());
    }

    #[test]
    fn prefers_user_token_for_photo_upload() {
        let settings = AppSettings {
            vk_token: "community".to_string(),
            vk_user_token: "user".to_string(),
            ..Default::default()
        };
        assert_eq!(photo_upload_token(&settings, "community"), "user");
    }

    #[test]
    fn falls_back_to_community_token_for_photo_upload() {
        let settings = AppSettings {
            vk_token: "community".to_string(),
            ..Default::default()
        };
        assert_eq!(photo_upload_token(&settings, "community"), "community");
    }

    #[test]
    fn accepts_http_link_attachment() {
        assert_eq!(
            link_attachment(Some("https://example.com/news")),
            Some("https://example.com/news".to_string())
        );
        assert!(link_attachment(Some("not-a-url")).is_none());
    }

    #[test]
    fn link_only_attachment_is_not_allowed() {
        assert!(build_attachments(
            None,
            Some("https://example.com/news")
        )
        .is_none());
    }

    #[test]
    fn photo_and_link_combined_in_attachments() {
        assert_eq!(
            build_attachments(
                Some("photo-1_2".to_string()),
                Some("https://example.com/news")
            ),
            Some("photo-1_2,https://example.com/news".to_string())
        );
    }

    #[test]
    fn appends_source_link_when_no_photo() {
        let msg = append_link_to_message("Заголовок\n\nТекст", Some("https://example.com/news"));
        assert!(msg.contains("https://example.com/news"));
        assert!(msg.starts_with("Заголовок"));
    }

    #[test]
    fn does_not_duplicate_link_in_message() {
        let msg = append_link_to_message(
            "Уже есть https://example.com/other",
            Some("https://example.com/news"),
        );
        assert_eq!(msg, "Уже есть https://example.com/other");
    }

    #[test]
    fn detects_group_auth_photo_error() {
        assert!(is_group_auth_photo_error(
            "Group authorization failed: method is unavailable with group auth."
        ));
    }
}
