use crate::services::data_dir;
use anyhow::{Context, Result};
use reqwest::Client;
use std::path::{Path, PathBuf};

pub const LOCAL_IMAGE_PREFIX: &str = "local:";

pub fn is_local_image_ref(image_ref: &str) -> bool {
    image_ref.starts_with(LOCAL_IMAGE_PREFIX)
}

pub fn local_image_relative_path(image_ref: &str) -> Option<&str> {
    image_ref.strip_prefix(LOCAL_IMAGE_PREFIX)
}

pub fn resolve_local_image_path(data_dir: &Path, image_ref: &str) -> Result<PathBuf> {
    let relative = local_image_relative_path(image_ref)
        .with_context(|| format!("Not a local image ref: {}", image_ref))?;
    let path = data_dir.join(relative);
    if !path.exists() {
        anyhow::bail!("Local image not found: {}", path.display());
    }
    Ok(path)
}

pub async fn load_image_bytes(
    client: &Client,
    data_dir: &Path,
    image_ref: &str,
) -> Result<Vec<u8>> {
    if is_local_image_ref(image_ref) {
        let path = resolve_local_image_path(data_dir, image_ref)?;
        return std::fs::read(&path).with_context(|| format!("Read {}", path.display()));
    }

    client
        .get(image_ref)
        .header(
            "User-Agent",
            "Mozilla/5.0 GamingNewsPublisher/0.1",
        )
        .send()
        .await
        .context("Image download failed")?
        .error_for_status()
        .context("Image HTTP error")?
        .bytes()
        .await
        .map(|b| b.to_vec())
        .context("Image body read failed")
}

pub fn clear_images_dir(data_dir: &Path) -> Result<()> {
    let images_dir = data_dir::images_dir(data_dir);
    if images_dir.exists() {
        std::fs::remove_dir_all(&images_dir)?;
    }
    std::fs::create_dir_all(&images_dir)?;
    Ok(())
}
