use crate::services::local_model_catalog::{ModelDefinition, ModelKind};
use crate::services::llm_dir;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModelRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub download_url: String,
    pub filename: String,
    pub size_hint_bytes: u64,
    pub min_vram_gb: u8,
    pub layer_count_hint: u32,
}

fn store_path() -> Result<PathBuf> {
    Ok(llm_dir::llm_root()?.join("custom_models.json"))
}

pub fn load_all() -> Result<Vec<CustomModelRecord>> {
    let path = store_path()?;
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let json = std::fs::read_to_string(&path)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&json).context("Invalid custom_models.json")?)
}

fn save_all(records: &[CustomModelRecord]) -> Result<()> {
    let path = store_path()?;
    let json = serde_json::to_string_pretty(records)?;
    std::fs::write(&path, json).with_context(|| format!("Cannot write {}", path.display()))?;
    Ok(())
}

impl CustomModelRecord {
    pub fn to_definition(&self) -> ModelDefinition {
        ModelDefinition {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            filename: self.filename.clone(),
            download_url: self.download_url.clone(),
            size_hint_bytes: self.size_hint_bytes,
            expected_sha256: None,
            min_vram_gb: self.min_vram_gb,
            layer_count_hint: self.layer_count_hint,
            recommended: false,
            deprecated_reason: None,
            is_custom: true,
            model_kind: ModelKind::Llm,
        }
    }
}

pub fn is_custom_model(id: &str) -> bool {
    load_all()
        .ok()
        .is_some_and(|records| records.iter().any(|r| r.id == id))
}

pub fn remove(id: &str) -> Result<bool> {
    let mut records = load_all()?;
    let before = records.len();
    records.retain(|r| r.id != id);
    if records.len() == before {
        return Ok(false);
    }
    save_all(&records)?;
    Ok(true)
}

pub fn update_size_hint(id: &str, size_bytes: u64) -> Result<()> {
    let mut records = load_all()?;
    let Some(record) = records.iter_mut().find(|r| r.id == id) else {
        return Ok(());
    };
    if record.size_hint_bytes == 0 {
        record.size_hint_bytes = size_bytes;
        save_all(&records)?;
    }
    Ok(())
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in input.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn unique_custom_id(base: &str, existing: &[CustomModelRecord]) -> String {
    let base = if base.is_empty() { "model".into() } else { base };
    let mut candidate = format!("custom-{base}");
    let mut n = 2;
    while existing.iter().any(|r| r.id == candidate) {
        candidate = format!("custom-{base}-{n}");
        n += 1;
    }
    candidate
}

pub fn parse_download_url(url: &str) -> Result<(String, String)> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") {
        bail!("URL должен начинаться с https://");
    }
    let parsed = reqwest::Url::parse(trimmed).context("Некорректный URL")?;
    let filename = parsed
        .path_segments()
        .and_then(|mut s| s.next_back().map(String::from))
        .filter(|f| !f.is_empty())
        .ok_or_else(|| anyhow!("Не удалось определить имя файла из URL"))?;
    if !filename.to_ascii_lowercase().ends_with(".gguf") {
        bail!("Файл должен иметь расширение .gguf");
    }
    Ok((filename, trimmed.to_string()))
}

pub async fn fetch_size_hint(http: &reqwest::Client, url: &str) -> u64 {
    http.head(url)
        .send()
        .await
        .ok()
        .and_then(|r| r.content_length())
        .unwrap_or(0)
}

pub async fn add(
    http: &reqwest::Client,
    name: String,
    description: String,
    download_url: String,
) -> Result<ModelDefinition> {
    let name = name.trim().to_string();
    if name.is_empty() {
        bail!("Укажите название модели");
    }
    let description = description.trim().to_string();
    let (filename, download_url) = parse_download_url(&download_url)?;

    let mut records = load_all()?;
    if records.iter().any(|r| r.filename.eq_ignore_ascii_case(&filename)) {
        bail!("Модель с таким файлом уже добавлена");
    }

    let size_hint_bytes = fetch_size_hint(http, &download_url).await;
    let id = unique_custom_id(&slugify(&name), &records);

    let record = CustomModelRecord {
        id: id.clone(),
        name,
        description,
        download_url,
        filename,
        size_hint_bytes,
        min_vram_gb: 6,
        layer_count_hint: 28,
    };
    let def = record.to_definition();
    records.push(record);
    save_all(&records)?;
    Ok(def)
}
