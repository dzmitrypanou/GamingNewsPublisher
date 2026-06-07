use crate::models::{ApiTestResult, AppSettings};
use crate::services::image_loader;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::path::Path;

pub async fn test_connection(client: &Client, settings: &AppSettings) -> ApiTestResult {
    if settings.vk_token.is_empty() || settings.vk_group_id.is_empty() {
        return ApiTestResult {
            success: false,
            message: "Токен или ID группы не указаны".to_string(),
        };
    }

    match get_group_info(client, settings).await {
        Ok(name) => ApiTestResult {
            success: true,
            message: format!("Подключено: {}", name),
        },
        Err(e) => ApiTestResult {
            success: false,
            message: format!("Ошибка: {}", e),
        },
    }
}

async fn get_group_info(client: &Client, settings: &AppSettings) -> Result<String> {
    let group_id = settings.vk_group_id.trim().trim_start_matches('-');
    let url = format!(
        "https://api.vk.com/method/groups.getById?group_id={}&access_token={}&v=5.199",
        group_id, settings.vk_token
    );
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;

    if let Some(err) = resp.get("error") {
        anyhow::bail!("{}", err["error_msg"].as_str().unwrap_or("VK API error"));
    }

    let name = resp["response"]["groups"][0]["name"]
        .as_str()
        .unwrap_or("Группа")
        .to_string();
    Ok(name)
}

pub async fn publish_post(
    client: &Client,
    settings: &AppSettings,
    message: &str,
    image_url: Option<&str>,
    data_dir: Option<&Path>,
) -> Result<String> {
    let group_id = settings.vk_group_id.trim().trim_start_matches('-');
    let owner_id = format!("-{}", group_id);

    let attachment = if let Some(img_url) = image_url {
        match upload_photo(client, settings, &owner_id, img_url, data_dir).await {
            Ok(att) => Some(att),
            Err(_) => None,
        }
    } else {
        None
    };

    let mut params = vec![
        ("owner_id", owner_id.clone()),
        ("from_group", "1".to_string()),
        ("message", message.to_string()),
        ("access_token", settings.vk_token.clone()),
        ("v", "5.199".to_string()),
    ];

    if let Some(ref att) = attachment {
        params.push(("attachments", att.clone()));
    }

    let resp: serde_json::Value = client
        .post("https://api.vk.com/method/wall.post")
        .form(&params)
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = resp.get("error") {
        anyhow::bail!("{}", err["error_msg"].as_str().unwrap_or("VK wall.post error"));
    }

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
    let group_id = settings.vk_group_id.trim().trim_start_matches('-');
    let owner_id = format!("-{}", group_id);

    let resp: serde_json::Value = client
        .post("https://api.vk.com/method/wall.delete")
        .form(&[
            ("owner_id", owner_id),
            ("post_id", post_id.to_string()),
            ("access_token", settings.vk_token.clone()),
            ("v", "5.199".to_string()),
        ])
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = resp.get("error") {
        anyhow::bail!("{}", err["error_msg"].as_str().unwrap_or("VK wall.delete error"));
    }

    Ok(())
}

async fn upload_photo(
    client: &Client,
    settings: &AppSettings,
    _owner_id: &str,
    image_url: &str,
    data_dir: Option<&Path>,
) -> Result<String> {
    let group_id = settings.vk_group_id.trim().trim_start_matches('-');

    let upload_server_url = format!(
        "https://api.vk.com/method/photos.getWallUploadServer?group_id={}&access_token={}&v=5.199",
        group_id, settings.vk_token
    );
    let server_resp: serde_json::Value = client.get(&upload_server_url).send().await?.json().await?;

    if let Some(err) = server_resp.get("error") {
        anyhow::bail!("{}", err["error_msg"].as_str().unwrap_or("VK upload server error"));
    }

    let upload_url = server_resp["response"]["upload_url"]
        .as_str()
        .context("No upload_url")?;

    let img_bytes = if let Some(dir) = data_dir {
        image_loader::load_image_bytes(client, dir, image_url).await?
    } else {
        client
            .get(image_url)
            .send()
            .await?
            .bytes()
            .await?
            .to_vec()
    };

    let form = reqwest::multipart::Form::new().part(
        "photo",
        reqwest::multipart::Part::bytes(img_bytes.to_vec())
            .file_name("photo.jpg")
            .mime_str("image/jpeg")?,
    );

    let upload_resp: serde_json::Value = client
        .post(upload_url)
        .multipart(form)
        .send()
        .await?
        .json()
        .await?;

    let save_params = json!({
        "group_id": group_id,
        "photo": upload_resp["photo"].as_str().unwrap_or(""),
        "server": upload_resp["server"].as_i64().unwrap_or(0),
        "hash": upload_resp["hash"].as_str().unwrap_or(""),
        "access_token": settings.vk_token,
        "v": "5.199"
    });

    let save_resp: serde_json::Value = client
        .post("https://api.vk.com/method/photos.saveWallPhoto")
        .form(&[
            ("group_id", save_params["group_id"].as_str().unwrap_or("")),
            ("photo", save_params["photo"].as_str().unwrap_or("")),
            ("server", &save_params["server"].to_string()),
            ("hash", save_params["hash"].as_str().unwrap_or("")),
            ("access_token", save_params["access_token"].as_str().unwrap_or("")),
            ("v", "5.199"),
        ])
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = save_resp.get("error") {
        anyhow::bail!("{}", err["error_msg"].as_str().unwrap_or("VK save photo error"));
    }

    let photo = &save_resp["response"][0];
    let photo_id = photo["id"].as_i64().context("No photo id")?;
    let photo_owner = photo["owner_id"].as_i64().context("No photo owner")?;

    Ok(format!("photo{}_{}", photo_owner, photo_id))
}

pub fn format_message(title: &str, text: &str, hashtags: &str) -> String {
    let bold_title = format!("**{}**", title.replace('*', ""));
    let mut parts = vec![bold_title, String::new(), text.to_string()];
    if !hashtags.is_empty() {
        parts.push(String::new());
        parts.push(hashtags.to_string());
    }
    let msg = parts.join("\n");
    if msg.len() > 4096 {
        msg.chars().take(4093).collect::<String>() + "..."
    } else {
        msg
    }
}
