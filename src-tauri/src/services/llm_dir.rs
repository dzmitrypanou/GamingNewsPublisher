use crate::services::local_model_catalog;

use anyhow::{Context, Result};

use std::path::{Path, PathBuf};



pub const LLAMA_SERVER_EXE: &str = "llama-server.exe";

pub const LLAMA_DLL: &str = "llama.dll";

pub const GGML_VULKAN_DLL: &str = "ggml-vulkan.dll";

const MIN_MODEL_BYTES_FALLBACK: u64 = 512 * 1024;

/// Допустимый размер скачанного файла (±2% от size_hint или fallback для неизвестных).
pub fn is_valid_model_size(model_id: &str, size: u64) -> bool {
    let Some(def) = local_model_catalog::find(model_id) else {
        return size >= MIN_MODEL_BYTES_FALLBACK;
    };
    if def.size_hint_bytes == 0 {
        return size >= MIN_MODEL_BYTES_FALLBACK;
    }
    let min = def.size_hint_bytes.saturating_mul(98) / 100;
    let max = def.size_hint_bytes.saturating_mul(102) / 100;
    size >= min && size <= max
}

pub fn dedup_model_selectable(model_id: &str) -> bool {
    let id = local_model_catalog::normalize_model_id(model_id);
    local_model_catalog::find(id).is_some_and(|def| {
        def.model_kind == local_model_catalog::ModelKind::Encoder && def.deprecated_reason.is_none()
    })
}

pub fn resolve_dedup_model_id(model_id: &str) -> String {
    let id = local_model_catalog::normalize_model_id(model_id);
    if dedup_model_selectable(id) && model_installed(id) {
        return id.to_string();
    }
    for fallback in ["bge-m3", "rubert-tiny2", "multilingual-e5-large-instruct"] {
        if dedup_model_selectable(fallback) && model_installed(fallback) {
            return fallback.to_string();
        }
    }
    id.to_string()
}



pub fn llm_root() -> Result<PathBuf> {

    let root = std::env::current_exe()

        .context("Failed to get executable path")?

        .parent()

        .context("Executable has no parent directory")?

        .join("llm");

    std::fs::create_dir_all(&root)?;

    Ok(root)

}



pub fn server_start_log_path() -> Result<PathBuf> {

    Ok(llm_root()?.join("last-server-start.log"))

}

pub fn embed_server_start_log_path() -> Result<PathBuf> {
    Ok(llm_root()?.join("last-embed-server-start.log"))
}



pub fn bin_dir() -> Result<PathBuf> {

    let dir = llm_root()?.join("bin");

    std::fs::create_dir_all(&dir)?;

    Ok(dir)

}



pub fn models_dir() -> Result<PathBuf> {

    let dir = llm_root()?.join("models");

    std::fs::create_dir_all(&dir)?;

    Ok(dir)

}



pub fn server_path() -> Result<PathBuf> {

    Ok(bin_dir()?.join(LLAMA_SERVER_EXE))

}



pub fn model_path_for(model_id: &str) -> Result<PathBuf> {

    let def = local_model_catalog::find(model_id)

        .with_context(|| format!("Unknown model id: {model_id}"))?;

    Ok(models_dir()?.join(def.filename))

}



pub fn partial_path_for(model_id: &str) -> Result<PathBuf> {

    Ok(model_path_for(model_id)?.with_extension("gguf.part"))

}



pub fn download_state_path_for(model_id: &str) -> Result<PathBuf> {

    Ok(llm_root()?.join(format!("download_state_{model_id}.json")))

}



pub fn partial_bytes_for(model_id: &str) -> u64 {

    partial_path_for(model_id)

        .ok()

        .and_then(|p| p.metadata().ok().map(|m| m.len()))

        .unwrap_or(0)

}



pub fn has_partial_download(model_id: &str) -> bool {

    !model_installed(model_id) && partial_bytes_for(model_id) > 1_000_000

}



pub fn model_file_invalid(model_id: &str) -> bool {

    model_path_for(model_id)

        .map(|p| p.is_file() && !model_file_valid(&p, model_id))

        .unwrap_or(false)

}



pub fn install_staging_dir() -> Result<PathBuf> {

    Ok(llm_root()?.join(".install-staging"))

}



pub fn server_installed() -> bool {

    match (server_path(), bin_dir()) {

        (Ok(exe), Ok(bin)) => {

            exe.is_file()

                && bin.join(LLAMA_DLL).is_file()

                && bin.join(GGML_VULKAN_DLL).is_file()

        }

        _ => false,

    }

}



pub fn model_installed(model_id: &str) -> bool {

    model_path_for(model_id)

        .map(|p| model_file_valid(&p, model_id))

        .unwrap_or(false)

}



fn model_file_valid(path: &Path, model_id: &str) -> bool {

    if !path.is_file() {

        return false;

    }

    let Ok(meta) = path.metadata() else {

        return false;

    };

    let size = meta.len();

    is_valid_model_size(model_id, size)

}



pub fn files_ready(active_model_id: &str) -> bool {

    server_installed() && model_installed(active_model_id)

}



pub fn any_model_installed() -> bool {

    local_model_catalog::all_models()

        .iter()

        .any(|m| model_installed(&m.id))

}



pub fn installed_model_ids() -> Vec<String> {

    local_model_catalog::all_models()

        .iter()

        .filter(|m| model_installed(&m.id))

        .map(|m| m.id.clone())

        .collect()

}



pub fn file_bytes_for_model(model_id: &str) -> u64 {

    model_path_for(model_id)

        .ok()

        .and_then(|p| p.metadata().ok().map(|m| m.len()))

        .unwrap_or(0)

}



pub fn remove_invalid_model_file(model_id: &str) -> Result<bool> {

    let path = model_path_for(model_id)?;

    if path.is_file() && !model_file_valid(&path, model_id) {

        std::fs::remove_file(&path)?;

        Ok(true)

    } else {

        Ok(false)

    }

}



pub fn disk_usage_bytes() -> i64 {

    let mut total = 0i64;

    if let Ok(root) = llm_root() {

        total += dir_size(&root);

    }

    total

}



fn dir_size(path: &Path) -> i64 {

    let mut total = 0i64;

    let Ok(entries) = std::fs::read_dir(path) else {

        return 0;

    };

    for entry in entries.flatten() {

        let path = entry.path();

        if path.is_file() {

            total += path.metadata().map(|m| m.len() as i64).unwrap_or(0);

        } else if path.is_dir() {

            total += dir_size(&path);

        }

    }

    total

}


