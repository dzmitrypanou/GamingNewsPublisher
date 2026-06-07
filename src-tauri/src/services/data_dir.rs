use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const DB_FILE: &str = "gaming_news.db";
const SETTINGS_FILE: &str = "settings.json";

pub fn resolve(app: &AppHandle) -> Result<PathBuf> {
    let data_dir = exe_data_dir()?;
    migrate_from_app_data(app, &data_dir)?;
    Ok(data_dir)
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE)
}

pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DB_FILE)
}

pub fn images_dir(data_dir: &Path) -> PathBuf {
    let dir = data_dir.join("images");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn watermark_dir(data_dir: &Path) -> PathBuf {
    let dir = data_dir.join("watermark");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn exe_data_dir() -> Result<PathBuf> {
    let data_dir = std::env::current_exe()
        .context("Failed to get executable path")?
        .parent()
        .context("Executable has no parent directory")?
        .join("data");
    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

fn migrate_from_app_data(app: &AppHandle, data_dir: &Path) -> Result<()> {
    if data_dir.join(DB_FILE).exists() || data_dir.join(SETTINGS_FILE).exists() {
        return Ok(());
    }

    let old_dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => return Ok(()),
    };

    if !old_dir.exists() {
        return Ok(());
    }

    for file in [DB_FILE, SETTINGS_FILE] {
        let from = old_dir.join(file);
        let to = data_dir.join(file);
        if from.exists() && !to.exists() {
            std::fs::copy(&from, &to)
                .with_context(|| format!("Failed to migrate {}", file))?;
        }
    }

    Ok(())
}
