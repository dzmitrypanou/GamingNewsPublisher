use crate::models::{ApiTestResult, AppSettings};
use crate::services::image_loader;
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

/// VK ID OAuth выдаёт `vk2.a.*` — такой токен не работает с классическим api.vk.com (photos.*, wall.*).
pub fn is_vk_id_api_token(token: &str) -> bool {
    token.trim().starts_with("vk2.a.")
}

pub fn vk_user_token_photo_hint(token: &str) -> Option<&'static str> {
    if is_vk_id_api_token(token) {
        Some(
            "Токен VK ID (vk2.a.*) обновляется автоматически перед публикацией, \
             если сохранён refresh token. Нужны одобренные права wall и photos в кабинете VK ID.",
        )
    } else {
        None
    }
}

fn is_invalid_access_token_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("invalid access_token") || lower.contains("access_token has expired")
}

fn photo_upload_failure_message(settings: &AppSettings, err: &anyhow::Error) -> String {
    let detail = err.to_string();
    if has_user_photo_token(settings) {
        if let Some(hint) = vk_user_token_photo_hint(&settings.vk_user_token) {
            return format!("Не удалось загрузить фото в VK: {detail}. {hint}");
        }
        if is_invalid_access_token_error(&detail) {
            return format!(
                "Не удалось загрузить фото в VK: {detail}. \
                 User token недействителен или истёк. Получите новый через «Получить user token» в настройках \
                 (права: wall, photos, offline)."
            );
        }
        return format!(
            "Не удалось загрузить фото в VK: {detail}. \
             Проверьте права user token (wall, photos, offline) и что аккаунт — админ/редактор группы."
        );
    }
    if is_group_auth_photo_error(&detail) || is_scope_error(&detail) {
        return format!(
            "Не удалось загрузить фото в VK: {detail}. \
             Ключ сообщества не может загружать фото (ограничение VK API). \
             Укажите user token в настройках — кнопка «Получить user token» (права «Стена», «Фотографии», «Offline»)."
        );
    }
    format!(
        "Не удалось загрузить фото в VK: {detail}. \
         Для публикации с картинкой нужен user token (кнопка «Получить user token»: wall + photos + offline)."
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

            if let Some(hint) = vk_user_token_photo_hint(&settings.vk_user_token) {
                // vk2.a с refresh token проверяем через API, не блокируем заранее
                if settings.vk_refresh_token.trim().is_empty() {
                    return ApiTestResult {
                        success: false,
                        message: format!("Группа «{name}» найдена. {hint}"),
                    };
                }
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
                    if is_invalid_access_token_error(&e.to_string()) {
                        return ApiTestResult {
                            success: false,
                            message: format!(
                                "Группа «{name}» найдена, но user token недействителен: {e}. \
                                 Получите новый токен через «Получить user token» (wall + photos + offline)."
                            ),
                        };
                    }
                    let scope_hint = if is_scope_error(&e.to_string()) {
                        match get_user_token_permissions(client, &upload_token).await {
                            Ok(mask) => format!(
                                " Не хватает прав: {}. В настройках VK отметьте «Фотографии» и «Стена» при получении токена.",
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
                         Получите user token кнопкой «Получить user token» (wall + photos + offline) \
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
    data_dir: Option<&Path>,
) -> Result<String> {
    let image_url = image_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .context("VK: пост без изображения не публикуется")?;

    let (community_token, group_id) = group_credentials(settings)?;
    let upload_token = photo_upload_token(settings, &community_token);
    let owner_id = format!("-{group_id}");

    let photo_attachment = upload_photo(client, &upload_token, &group_id, image_url, data_dir)
        .await
        .map_err(|e| anyhow::anyhow!(photo_upload_failure_message(settings, &e)))?;

    let params = vec![
        ("owner_id", owner_id),
        ("from_group", "1".to_string()),
        ("message", message.to_string()),
        ("attachments", photo_attachment),
        ("access_token", community_token),
    ];

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
    } else if image_loader::is_local_image_ref(image_url) {
        bail!("Local image requires data_dir");
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
    let saved_owner = photo_obj["owner_id"].as_i64().context("No photo owner")?;

    Ok(group_wall_photo_attachment(group_id, photo_id, saved_owner))
}

/// VK возвращает owner_id пользователя, но для wall.post сообщества нужен `-group_id`.
fn group_wall_photo_attachment(group_id: &str, photo_id: i64, saved_owner: i64) -> String {
    if saved_owner < 0 {
        format!("photo{saved_owner}_{photo_id}")
    } else {
        format!("photo-{group_id}_{photo_id}")
    }
}

pub fn format_message(title: &str, text: &str, hashtags: &str) -> String {
    let title = title.trim().replace('*', "");

    let mut parts = Vec::new();
    if !title.is_empty() {
        parts.push(title.to_string());
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
    fn detects_vk_id_token_prefix() {
        assert!(is_vk_id_api_token("vk2.a.abc"));
        assert!(!is_vk_id_api_token("vk1.a.abc"));
        assert!(vk_user_token_photo_hint("vk2.a.x").is_some());
        assert!(vk_user_token_photo_hint("vk1.a.x").is_none());
    }

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
    fn group_wall_photo_attachment_uses_negative_group_id() {
        assert_eq!(
            group_wall_photo_attachment("225364560", 456, 1468099),
            "photo-225364560_456"
        );
        assert_eq!(
            group_wall_photo_attachment("225364560", 456, -225364560),
            "photo-225364560_456"
        );
    }

    #[test]
    fn format_message_uses_plain_title_without_asterisks() {
        let msg = format_message("Destiny 2: тест", "Текст поста", "#игры");
        assert!(msg.starts_with("Destiny 2: тест"));
        assert!(!msg.contains('*'));
        assert!(msg.contains("Текст поста"));
        assert!(msg.contains("#игры"));
    }

    #[test]
    fn detects_group_auth_photo_error() {
        assert!(is_group_auth_photo_error(
            "Group authorization failed: method is unavailable with group auth."
        ));
    }
}
