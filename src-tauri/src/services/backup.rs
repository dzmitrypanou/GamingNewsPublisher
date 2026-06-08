use crate::models::BackupExportResult;
use crate::services::data_dir;
use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const BACKUP_MANIFEST: &str = "manifest.json";
pub const BACKUP_VERSION: u32 = 1;
const DB_FILE: &str = "gaming_news.db";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    version: u32,
    created_at: String,
    app_version: String,
}

pub fn default_backup_filename() -> String {
    let now = Local::now();
    format!(
        "gaming-news-backup_{}.zip",
        now.format("%Y-%m-%d_%H-%M")
    )
}

pub fn export_backup(data_dir: &Path, dest_zip: &Path) -> Result<BackupExportResult> {
    if let Some(parent) = dest_zip.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_zip = dest_zip.with_extension("zip.part");
    if temp_zip.exists() {
        fs::remove_file(&temp_zip)?;
    }

    let file = File::create(&temp_zip).with_context(|| format!("Create {}", temp_zip.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let manifest = BackupManifest {
        version: BACKUP_VERSION,
        created_at: Local::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    zip.start_file(BACKUP_MANIFEST, options)?;
    zip.write_all(manifest_json.as_bytes())?;

    let db_path = data_dir::database_path(data_dir);
    if db_path.exists() {
        add_file_to_zip(&mut zip, &db_path, DB_FILE, options)?;
    } else {
        anyhow::bail!("База данных не найдена");
    }

    let settings_path = data_dir::settings_path(data_dir);
    if settings_path.exists() {
        add_file_to_zip(&mut zip, &settings_path, SETTINGS_FILE, options)?;
    }

    add_dir_to_zip(&mut zip, &data_dir::images_dir(data_dir), "images", options)?;
    add_dir_to_zip(&mut zip, &data_dir::watermark_dir(data_dir), "watermark", options)?;

    zip.finish()?;

    if dest_zip.exists() {
        fs::remove_file(dest_zip)?;
    }
    fs::rename(&temp_zip, dest_zip)?;

    let size_bytes = fs::metadata(dest_zip)?.len();
    Ok(BackupExportResult {
        path: dest_zip.display().to_string(),
        size_bytes,
    })
}

pub fn import_backup(data_dir: &Path, zip_path: &Path) -> Result<()> {
    let file = File::open(zip_path).with_context(|| format!("Open {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file).context("Некорректный ZIP-архив")?;

    let mut manifest: Option<BackupManifest> = None;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.name() == BACKUP_MANIFEST || entry.name().ends_with(BACKUP_MANIFEST) {
            let mut buf = String::new();
            entry.read_to_string(&mut buf)?;
            manifest = Some(serde_json::from_str(&buf).context("manifest.json")?);
            break;
        }
    }

    let manifest = manifest.context("В архиве нет manifest.json")?;
    if manifest.version != BACKUP_VERSION {
        anyhow::bail!(
            "Неподдерживаемая версия бэкапа: {}",
            manifest.version
        );
    }

    let temp_root = data_dir.join(".backup_restore_tmp");
    if temp_root.exists() {
        remove_dir_all(&temp_root)?;
    }
    fs::create_dir_all(&temp_root)?;

    let mut has_db = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().replace('\\', "/");
        if name == BACKUP_MANIFEST || name.ends_with(BACKUP_MANIFEST) {
            continue;
        }
        if name.contains("..") {
            anyhow::bail!("Недопустимый путь в архиве: {name}");
        }

        let out_path = safe_join(&temp_root, &name)?;
        if name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
        if name == DB_FILE {
            has_db = true;
        }
    }

    if !has_db {
        remove_dir_all(&temp_root)?;
        anyhow::bail!("В архиве нет gaming_news.db");
    }

    fs::create_dir_all(data_dir)?;

    replace_file(
        &temp_root.join(DB_FILE),
        &data_dir::database_path(data_dir),
    )?;
    remove_sqlite_sidecars(&data_dir::database_path(data_dir));

    let settings_src = temp_root.join(SETTINGS_FILE);
    if settings_src.exists() {
        replace_file(&settings_src, &data_dir::settings_path(data_dir))?;
    }

    replace_tree(&temp_root.join("images"), &data_dir::images_dir(data_dir))?;
    replace_tree(
        &temp_root.join("watermark"),
        &data_dir::watermark_dir(data_dir),
    )?;

    remove_dir_all(&temp_root)?;
    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    path: &Path,
    zip_name: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    zip.start_file(zip_name, options)?;
    let mut file = File::open(path)?;
    std::io::copy(&mut file, zip)?;
    Ok(())
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    dir: &Path,
    zip_prefix: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in walk_dir(dir)? {
        let relative = entry
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let zip_name = if zip_prefix.is_empty() {
            relative
        } else if relative.is_empty() {
            continue;
        } else {
            format!("{zip_prefix}/{relative}")
        };
        if entry.is_file() {
            add_file_to_zip(zip, &entry, &zip_name, options)?;
        }
    }
    Ok(())
}

fn walk_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_files(dir, &mut out)?;
    Ok(out)
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn safe_join(base: &Path, name: &str) -> Result<PathBuf> {
    let mut path = base.to_path_buf();
    for part in Path::new(name).components() {
        match part {
            Component::Normal(p) => path.push(p),
            Component::CurDir => {}
            _ => anyhow::bail!("Недопустимый путь в архиве: {name}"),
        }
    }
    if !path.starts_with(base) {
        anyhow::bail!("Недопустимый путь в архиве: {name}");
    }
    Ok(path)
}

fn replace_file(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = to.with_extension("restore.part");
    if temp.exists() {
        fs::remove_file(&temp)?;
    }
    fs::copy(from, &temp)?;
    if to.exists() {
        fs::remove_file(to)?;
    }
    fs::rename(&temp, to)?;
    Ok(())
}

fn replace_tree(from: &Path, to: &Path) -> Result<()> {
    if from.exists() {
        if to.exists() {
            remove_dir_all(to)?;
        }
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_dir_recursive(from, to)?;
    } else if to.exists() {
        remove_dir_all(to)?;
        fs::create_dir_all(to)?;
    }
    Ok(())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn remove_dir_all(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn remove_sqlite_sidecars(db_path: &Path) {
    let _ = fs::remove_file(db_path.with_extension("db-wal"));
    let _ = fs::remove_file(db_path.with_extension("db-shm"));
    let _ = fs::remove_file(format!("{}-wal", db_path.display()));
    let _ = fs::remove_file(format!("{}-shm", db_path.display()));
}
