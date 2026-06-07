use crate::local_llm_runtime::{DownloadSnapshot, LocalLlmRuntime};
use crate::models::AppSettings;
use crate::AppState;
use crate::services::{llm_dir, local_llm_overview, local_model_catalog};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use zip::ZipArchive;

const LLAMA_CPP_TAG: &str = "b4897";
const LLAMA_SERVER_ZIP_URL: &str =
    "https://github.com/ggml-org/llama.cpp/releases/download/b4897/llama-b4897-bin-win-vulkan-x64.zip";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) GamingNewsPublisher/0.1";

fn build_download_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(60))
        .read_timeout(Duration::from_secs(120))
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(10))
        .no_proxy()
        .build()
        .context("Не удалось создать HTTP-клиент для загрузки")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedDownloadState {
    model_id: String,
    stage: String,
    bytes_done: u64,
    bytes_total: u64,
}

fn emit_progress(app: &AppHandle, runtime: &LocalLlmRuntime) {
    let overview = app
        .try_state::<Arc<AppState>>()
        .map(|state| {
            local_llm_overview::build_overview_with_embed(
                app,
                runtime,
                Some(&state.local_embed),
                None,
            )
        })
        .unwrap_or_else(|| local_llm_overview::build_overview(app, runtime));
    let _ = app.emit("local-llm-download-progress", overview);
}

pub fn emit_overview(app: &AppHandle, runtime: &LocalLlmRuntime) {
    emit_progress(app, runtime);
}

fn persist_model_state(model_id: &str, stage: &str, bytes_done: u64, bytes_total: u64) {
    let state = PersistedDownloadState {
        model_id: model_id.to_string(),
        stage: stage.to_string(),
        bytes_done,
        bytes_total,
    };
    if let Ok(path) = llm_dir::download_state_path_for(model_id) {
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn clear_model_state(model_id: &str) {
    if let Ok(path) = llm_dir::download_state_path_for(model_id) {
        let _ = std::fs::remove_file(path);
    }
}

fn update_model_progress(
    runtime: &LocalLlmRuntime,
    model_id: &str,
    stage: &str,
    bytes_done: u64,
    bytes_total: u64,
    error: Option<String>,
) {
    if let Ok(mut guard) = runtime.downloads.lock() {
        guard.update_model(
            model_id,
            DownloadSnapshot {
                bytes_done,
                bytes_total,
                stage: stage.to_string(),
                error,
            },
        );
    }
}

fn update_server_progress(
    runtime: &LocalLlmRuntime,
    stage: &str,
    bytes_done: u64,
    bytes_total: u64,
    error: Option<String>,
) {
    if let Ok(mut guard) = runtime.downloads.lock() {
        guard.update_server(DownloadSnapshot {
            bytes_done,
            bytes_total,
            stage: stage.to_string(),
            error,
        });
    }
}

fn is_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::SeqCst)
}

fn check_cancelled(cancel: &AtomicBool) -> Result<()> {
    if is_cancelled(cancel) {
        Err(anyhow!("cancelled"))
    } else {
        Ok(())
    }
}

pub fn start_server_download(app: AppHandle, runtime: Arc<LocalLlmRuntime>) {
    if llm_dir::server_installed() {
        return;
    }

    let cancel = {
        let mut guard = runtime.downloads.lock().unwrap();
        match guard.try_start_server() {
            Some(c) => c,
            None => return,
        }
    };

    emit_progress(&app, &runtime);

    tauri::async_runtime::spawn(async move {
        let http = match build_download_client() {
            Ok(c) => c,
            Err(e) => {
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.set_server_error(e.to_string());
                }
                emit_progress(&app, &runtime);
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_server();
                }
                emit_progress(&app, &runtime);
                return;
            }
        };

        let result = download_llama_server(&app, &runtime, &http, &cancel).await;
        match result {
            Ok(()) => {
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_server();
                }
                emit_progress(&app, &runtime);
            }
            Err(e) if e.to_string() == "cancelled" => {
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_server();
                }
                emit_progress(&app, &runtime);
            }
            Err(e) => {
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.set_server_error(e.to_string());
                    guard.finish_server();
                }
                emit_progress(&app, &runtime);
            }
        }
    });
}

pub fn start_model_download(app: AppHandle, runtime: Arc<LocalLlmRuntime>, model_id: String) {
    let model_id = local_model_catalog::normalize_model_id(&model_id).to_string();
    if local_model_catalog::find(&model_id).is_none() {
        return;
    }
    if llm_dir::model_installed(&model_id) {
        return;
    }

    let cancel = {
        let mut guard = runtime.downloads.lock().unwrap();
        match guard.try_start_model(&model_id) {
            Some(c) => c,
            None => return,
        }
    };

    emit_progress(&app, &runtime);

    let model_id_spawn = model_id.clone();
    tauri::async_runtime::spawn(async move {
        let http = match build_download_client() {
            Ok(c) => c,
            Err(e) => {
                update_model_progress(&runtime, &model_id_spawn, "error", 0, 0, Some(e.to_string()));
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_model(&model_id_spawn);
                }
                emit_progress(&app, &runtime);
                return;
            }
        };

        let result = run_model_download(&app, &runtime, &http, &model_id_spawn, &cancel).await;
        match result {
            Ok(()) => {
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_model(&model_id_spawn);
                }
                emit_progress(&app, &runtime);
                maybe_start_active_llm(&app, &runtime).await;
            }
            Err(e) if e.to_string() == "cancelled" => {
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_model(&model_id_spawn);
                }
                emit_progress(&app, &runtime);
            }
            Err(e) => {
                update_model_progress(
                    &runtime,
                    &model_id_spawn,
                    "error",
                    0,
                    0,
                    Some(e.to_string()),
                );
                if let Ok(mut guard) = runtime.downloads.lock() {
                    guard.finish_model(&model_id_spawn);
                }
                emit_progress(&app, &runtime);
            }
        }
    });
}

pub fn pause_model_download(app: &AppHandle, runtime: &LocalLlmRuntime, model_id: &str) -> bool {
    cancel_model_download(app, runtime, model_id)
}

pub fn cancel_model_download(app: &AppHandle, runtime: &LocalLlmRuntime, model_id: &str) -> bool {
    let model_id = local_model_catalog::normalize_model_id(model_id);
    let cancelled = runtime
        .downloads
        .lock()
        .map(|mut g| g.cancel_model(model_id))
        .unwrap_or(false);
    if cancelled {
        emit_progress(app, runtime);
    }
    cancelled
}

pub fn cancel_server_download(app: &AppHandle, runtime: &LocalLlmRuntime) -> bool {
    let cancelled = runtime
        .downloads
        .lock()
        .map(|mut g| g.cancel_server())
        .unwrap_or(false);
    if cancelled {
        emit_progress(app, runtime);
    }
    cancelled
}

async fn maybe_start_active_llm(app: &AppHandle, runtime: &LocalLlmRuntime) {
    if let Ok(settings) = crate::services::settings_store::load_settings(app) {
        if settings.local_llm_needed()
            && llm_dir::files_ready(&settings.normalized_local_model_id())
            && !runtime.is_server_running()
        {
            if let Err(e) = runtime.start(&settings).await {
                eprintln!("Local LLM start after download: {}", e);
            }
            emit_progress(app, runtime);
        }
    }
}

async fn run_model_download(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    http: &reqwest::Client,
    model_id: &str,
    cancel: &AtomicBool,
) -> Result<()> {
    ensure_server_installed(app, runtime, http, cancel).await?;
    check_cancelled(cancel)?;

    if !llm_dir::model_installed(model_id) {
        download_model(app, runtime, http, model_id, cancel).await?;
    }

    clear_model_state(model_id);
    Ok(())
}

async fn ensure_server_installed(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    http: &reqwest::Client,
    cancel: &AtomicBool,
) -> Result<()> {
    if llm_dir::server_installed() {
        return Ok(());
    }

    let install_lock = runtime.server_install_lock();
    let _guard = install_lock.lock().await;

    if llm_dir::server_installed() {
        return Ok(());
    }

    download_llama_server(app, runtime, http, cancel).await
}

pub fn delete_partial_download(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    model_id: &str,
) -> Result<()> {
    local_model_catalog::find(model_id).context("Unknown model")?;

    let model_id = local_model_catalog::normalize_model_id(model_id);
    if llm_dir::model_installed(model_id) {
        anyhow::bail!("Модель уже установлена");
    }

    cancel_model_download(app, runtime, model_id);

    let _ = llm_dir::remove_invalid_model_file(model_id);

    let partial = llm_dir::partial_path_for(model_id)?;
    if partial.is_file() {
        std::fs::remove_file(&partial)?;
    }
    clear_model_state(model_id);
    emit_progress(app, runtime);
    Ok(())
}

pub fn delete_local_model(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    settings: &AppSettings,
    model_id: &str,
) -> Result<Option<AppSettings>> {
    local_model_catalog::find(model_id).context("Unknown model")?;

    let model_id = local_model_catalog::normalize_model_id(model_id);
    cancel_model_download(app, runtime, model_id);

    let active = settings.normalized_local_model_id();
    let active_dedup = settings.normalized_local_dedup_model_id();
    let deleting_active =
        active == model_id || active_dedup == model_id;

    if deleting_active {
        let others: Vec<_> = llm_dir::installed_model_ids()
            .into_iter()
            .filter(|id| id != model_id)
            .collect();
        if others.is_empty() {
            anyhow::bail!("Нельзя удалить единственную установленную модель");
        }
        if deleting_active {
            runtime.shutdown();
        }
    }

    let model_path = llm_dir::model_path_for(model_id)?;
    let partial = llm_dir::partial_path_for(model_id)?;
    if model_path.is_file() {
        std::fs::remove_file(&model_path)?;
    }
    if partial.is_file() {
        let _ = std::fs::remove_file(&partial);
    }
    clear_model_state(model_id);

    if crate::services::custom_model_store::is_custom_model(model_id) {
        let _ = crate::services::custom_model_store::remove(model_id);
    }

    let restart_settings = if deleting_active {
        let next_id = llm_dir::installed_model_ids()
            .into_iter()
            .find(|id| {
                id != model_id
                    && local_model_catalog::find(id)
                        .is_some_and(|_| local_model_catalog::llm_model_selectable(id))
            })
            .or_else(|| {
                llm_dir::installed_model_ids()
                    .into_iter()
                    .find(|id| id != model_id)
            })
            .context("No other installed model")?;
        let mut new_settings = settings.clone();
        new_settings.local_model_id = next_id.clone();
        new_settings.local_dedup_model_id = next_id;
        crate::services::settings_store::save_settings(app, &new_settings)?;
        Some(new_settings)
    } else {
        None
    };

    emit_progress(app, runtime);
    Ok(restart_settings)
}

async fn download_llama_server(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    http: &reqwest::Client,
    cancel: &AtomicBool,
) -> Result<()> {
    update_server_progress(runtime, "server", 0, 0, None);
    emit_progress(app, runtime);

    check_cancelled(cancel)?;
    runtime.stop_for_install();
    tokio::time::sleep(Duration::from_millis(800)).await;
    check_cancelled(cancel)?;

    let response = http
        .get(LLAMA_SERVER_ZIP_URL)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .context("Не удалось скачать llama-server")?;

    check_cancelled(cancel)?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "llama-server HTTP {}: {}",
            status,
            body.chars().take(120).collect::<String>()
        );
    }

    let zip_bytes = response
        .bytes()
        .await
        .context("Не удалось прочитать архив llama-server")?;

    check_cancelled(cancel)?;

    if zip_bytes.len() < 4 || zip_bytes[0..2] != [0x50, 0x4B] {
        anyhow::bail!(
            "Скачанный файл не является ZIP ({} байт).",
            zip_bytes.len()
        );
    }

    let total = zip_bytes.len() as u64;
    update_server_progress(runtime, "server", total, total, None);
    emit_progress(app, runtime);

    let cursor = std::io::Cursor::new(zip_bytes.as_ref());
    let mut archive = ZipArchive::new(cursor)
        .with_context(|| format!("Не удалось открыть ZIP llama-server ({LLAMA_CPP_TAG})"))?;

    check_cancelled(cancel)?;

    let bin_dir = llm_dir::bin_dir()?;
    let staging = llm_dir::install_staging_dir()?;
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging)?;

    extract_llama_server_from_zip(&mut archive, &staging)?;
    check_cancelled(cancel)?;
    promote_staging_to_bin(runtime, &staging, &bin_dir)?;

    Ok(())
}

fn promote_staging_to_bin(
    runtime: &LocalLlmRuntime,
    staging: &Path,
    bin_dir: &Path,
) -> Result<()> {
    runtime.stop_for_install();
    std::thread::sleep(Duration::from_millis(500));

    std::fs::create_dir_all(bin_dir)?;
    clear_bin_server_files(bin_dir)?;
    std::thread::sleep(Duration::from_millis(300));

    for entry in std::fs::read_dir(staging).context("Cannot read install staging")? {
        let entry = entry?;
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let dest = bin_dir.join(entry.file_name());
        copy_with_retry(&src, &dest, 8)?;
    }

    let _ = std::fs::remove_dir_all(staging);
    Ok(())
}

fn clear_bin_server_files(bin_dir: &Path) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(bin_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == llm_dir::LLAMA_SERVER_EXE || name.ends_with(".dll") {
            remove_with_retry(&path, 8)?;
        }
    }
    Ok(())
}

fn copy_with_retry(src: &Path, dest: &Path, retries: u32) -> Result<()> {
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..=retries {
        match std::fs::copy(src, dest) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt < retries {
                    std::thread::sleep(Duration::from_millis(400 * (attempt as u64 + 1)));
                }
            }
        }
    }
    Err(last_err.unwrap()).with_context(|| {
        format!(
            "Не удалось записать {}. Закройте другие копии приложения и попробуйте снова.",
            dest.display()
        )
    })
}

fn remove_with_retry(path: &Path, retries: u32) -> Result<()> {
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..=retries {
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt < retries {
                    std::thread::sleep(Duration::from_millis(400 * (attempt as u64 + 1)));
                }
            }
        }
    }
    Err(last_err.unwrap())
        .with_context(|| format!("Cannot remove {}", path.display()))
}

fn extract_llama_server_from_zip(
    archive: &mut ZipArchive<impl std::io::Read + std::io::Seek>,
    dest_dir: &Path,
) -> Result<()> {
    let mut found_exe = false;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Zip entry read failed")?;
        let name = file.name().replace('\\', "/");
        let Some(base) = name.rsplit('/').next() else {
            continue;
        };

        let extract = base == llm_dir::LLAMA_SERVER_EXE || base.ends_with(".dll");
        if !extract {
            continue;
        }

        let target = dest_dir.join(base);
        let mut out = std::fs::File::create(&target)
            .with_context(|| format!("Cannot write {}", target.display()))?;
        std::io::copy(&mut file, &mut out)
            .with_context(|| format!("Failed to extract {}", base))?;

        if base == llm_dir::LLAMA_SERVER_EXE {
            found_exe = true;
        }
    }

    if !found_exe {
        anyhow::bail!("llama-server.exe not found in release zip ({LLAMA_CPP_TAG})");
    }
    if !dest_dir.join(llm_dir::LLAMA_DLL).is_file() {
        anyhow::bail!("llama.dll not found in release zip ({LLAMA_CPP_TAG})");
    }
    if !dest_dir.join(llm_dir::GGML_VULKAN_DLL).is_file() {
        anyhow::bail!("ggml-vulkan.dll not found in release zip ({LLAMA_CPP_TAG})");
    }

    Ok(())
}

async fn download_model(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    http: &reqwest::Client,
    model_id: &str,
    cancel: &AtomicBool,
) -> Result<()> {
    let def = local_model_catalog::find(model_id).context("Unknown model")?;
    let model_path = llm_dir::model_path_for(model_id)?;
    let partial = llm_dir::partial_path_for(model_id)?;
    let model_url = def.download_url;

    if model_path.is_file() && !llm_dir::model_installed(model_id) {
        std::fs::remove_file(&model_path).with_context(|| {
            format!(
                "Не удалось удалить повреждённый файл {}",
                model_path.display()
            )
        })?;
    }

    if partial.is_file() {
        let partial_size = partial.metadata().map(|m| m.len()).unwrap_or(0);
        if llm_dir::is_valid_model_size(model_id, partial_size) {
            std::fs::rename(&partial, &model_path).or_else(|_| {
                std::fs::copy(&partial, &model_path)?;
                std::fs::remove_file(&partial)?;
                Ok::<(), std::io::Error>(())
            })?;
            update_model_progress(runtime, model_id, "model", partial_size, partial_size, None);
            emit_progress(app, runtime);
            return Ok(());
        }
    }

    let mut existing = if partial.is_file() {
        partial.metadata().map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    check_cancelled(cancel)?;

    let mut request = http
        .get(&model_url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "*/*");
    if existing > 0 {
        request = request.header("Range", format!("bytes={existing}-"));
    }

    let mut response = request
        .send()
        .await
        .with_context(|| format!("Не удалось подключиться к HuggingFace ({model_url})"))?;

    check_cancelled(cancel)?;

    let status = response.status();
    if status.as_u16() == 416 {
        let _ = std::fs::remove_file(&partial);
        existing = 0;
        response = http
            .get(&model_url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .context("Повторная загрузка модели после сброса")?;
    } else if !status.is_success() && status.as_u16() != 206 {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "HuggingFace HTTP {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        );
    }

    let status = response.status();
    let append = existing > 0 && status.as_u16() == 206;

    if existing > 0 && !append && status.is_success() {
        existing = 0;
    }

    let content_len = response.content_length().unwrap_or(0);
    let total = if append {
        existing + content_len
    } else if content_len > 0 {
        content_len
    } else {
        def.size_hint_bytes
    };

    update_model_progress(runtime, model_id, "model", existing, total, None);
    persist_model_state(model_id, "model", existing, total);
    emit_progress(app, runtime);

    let mut file = if append {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&partial)
            .with_context(|| format!("Cannot open {}", partial.display()))?
    } else {
        std::fs::File::create(&partial)
            .with_context(|| format!("Cannot create {}", partial.display()))?
    };

    let mut downloaded = existing;
    let mut last_emit = downloaded;

    loop {
        check_cancelled(cancel)?;

        match response.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk)
                    .with_context(|| format!("Ошибка записи в {}", partial.display()))?;
                downloaded += chunk.len() as u64;
                update_model_progress(
                    runtime,
                    model_id,
                    "model",
                    downloaded,
                    total.max(downloaded),
                    None,
                );
                if downloaded.saturating_sub(last_emit) >= 5 * 1024 * 1024 {
                    last_emit = downloaded;
                    persist_model_state(model_id, "model", downloaded, total);
                    emit_progress(app, runtime);
                }
            }
            Ok(None) => break,
            Err(e) => {
                file.sync_all().ok();
                anyhow::bail!(
                    "Обрыв загрузки на {} МБ: {}. Нажмите «Скачать» снова — продолжится с места остановки.",
                    downloaded / 1_048_576,
                    e
                );
            }
        }
    }

    check_cancelled(cancel)?;

    file.sync_all()?;
    drop(file);

    if !llm_dir::is_valid_model_size(model_id, downloaded) {
        anyhow::bail!(
            "Размер файла {} байт не совпадает с ожидаемым ~{} — скачайте модель заново",
            downloaded,
            def.size_hint_bytes
        );
    }

    if let Some(expected) = def.expected_sha256.as_deref() {
        verify_file_sha256(&partial, expected)?;
    }

    std::fs::rename(&partial, &model_path).or_else(|_| {
        std::fs::copy(&partial, &model_path)?;
        std::fs::remove_file(&partial)?;
        Ok::<(), std::io::Error>(())
    })?;

    update_model_progress(runtime, model_id, "model", downloaded, downloaded, None);
    emit_progress(app, runtime);

    if def.is_custom {
        let _ = crate::services::custom_model_store::update_size_hint(model_id, downloaded);
    }

    Ok(())
}

fn verify_file_sha256(path: &Path, expected: &str) -> Result<()> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Cannot open {} for checksum", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Cannot read {} for checksum", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = format!("{:x}", hasher.finalize());
    if digest != expected {
        let _ = std::fs::remove_file(path);
        anyhow::bail!(
            "Контрольная сумма не совпадает — файл повреждён, загрузка начнётся заново"
        );
    }
    Ok(())
}

pub fn copy_bundled_server_if_present() -> Result<()> {
    if llm_dir::server_installed() {
        return Ok(());
    }

    let bundled = Path::new("resources/llm").join(llm_dir::LLAMA_SERVER_EXE);
    if !bundled.is_file() {
        return Ok(());
    }

    let target = llm_dir::server_path()?;
    std::fs::copy(&bundled, &target)?;
    Ok(())
}
