use crate::local_llm_runtime::{LocalLlmRuntime, snapshot_progress_pct};
use crate::models::{LocalModelInfo, LocalModelsOverview};
use crate::services::{llm_dir, local_model_catalog, settings_store};
use tauri::AppHandle;

fn partial_progress_pct(partial_bytes: u64, size_hint: u64) -> f64 {
    if size_hint == 0 {
        return 0.0;
    }
    (partial_bytes as f64 / size_hint as f64 * 100.0).min(99.0)
}

pub fn build_overview(app: &AppHandle, runtime: &LocalLlmRuntime) -> LocalModelsOverview {
    build_overview_with_embed(app, runtime, None, None)
}

pub fn build_overview_with_embed(
    app: &AppHandle,
    runtime: &LocalLlmRuntime,
    embed_runtime: Option<&crate::local_embed_runtime::LocalEmbedRuntime>,
    embed_error: Option<String>,
) -> LocalModelsOverview {
    let settings = settings_store::load_settings(app).unwrap_or_default();
    let active_id = settings.normalized_local_model_id();
    let active_dedup_id = settings.normalized_local_dedup_model_id();
    let downloads = runtime.downloads.lock().unwrap();
    let server_snapshot = downloads.server_snapshot();
    let server_downloading = server_snapshot.is_some();
    let server_progress_pct = server_snapshot
        .as_ref()
        .map(snapshot_progress_pct)
        .unwrap_or(0.0);
    let server_download_error = server_snapshot.as_ref().and_then(|s| s.error.clone());

    let models: Vec<LocalModelInfo> = local_model_catalog::all_models()
        .iter()
        .map(|def| {
            let snapshot = downloads.model_snapshot(&def.id);
            let downloading = snapshot.is_some();
            let installed = llm_dir::model_installed(&def.id);
            let install_invalid = llm_dir::model_file_invalid(&def.id);
            let partial_bytes = if installed {
                0
            } else {
                llm_dir::partial_bytes_for(&def.id)
            };
            let has_partial_download = llm_dir::has_partial_download(&def.id);
            let progress_pct = if downloading {
                snapshot
                    .as_ref()
                    .map(snapshot_progress_pct)
                    .unwrap_or(0.0)
            } else if has_partial_download {
                partial_progress_pct(partial_bytes, def.size_hint_bytes)
            } else {
                0.0
            };
            let download_error = snapshot.and_then(|s| s.error);

            LocalModelInfo {
                id: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                size_hint_bytes: def.size_hint_bytes,
                min_vram_gb: def.min_vram_gb,
                layer_count_hint: def.layer_count_hint,
                recommended: def.recommended,
                deprecated_reason: def.deprecated_reason.clone(),
                installed,
                install_invalid,
                file_bytes: if installed {
                    llm_dir::file_bytes_for_model(&def.id)
                } else {
                    partial_bytes
                },
                is_active: def.id == active_id,
                downloading,
                has_partial_download,
                progress_pct,
                download_error,
                is_custom: def.is_custom,
                model_kind: def.model_kind.as_str().to_string(),
                is_active_dedup: def.id == active_dedup_id,
            }
        })
        .collect();

    let any_model_downloading = models.iter().any(|m| m.downloading);
    let downloading = server_downloading || any_model_downloading;
    let download_model_id = models
        .iter()
        .find(|m| m.downloading)
        .map(|m| m.id.clone());

    let files_ready = llm_dir::files_ready(&active_id);
    let ready = runtime.is_ready(&settings);
    let dedup_files_ready = llm_dir::model_installed(&active_dedup_id) && llm_dir::server_installed();
    let dedup_ready = embed_runtime
        .map(|e| e.is_ready(&active_dedup_id))
        .unwrap_or(false);

    let runtime_error = if files_ready && !ready {
        runtime.last_start_error()
    } else if llm_dir::model_file_invalid(&active_id) {
        Some("Файл модели повреждён или неполный. Удалите и скачайте заново.".into())
    } else {
        None
    };

    let dedup_runtime_error = if settings.local_embed_needed() && dedup_files_ready && !dedup_ready {
        embed_runtime
            .and_then(|e| e.last_start_error())
            .or(embed_error)
    } else if llm_dir::model_file_invalid(&active_dedup_id) {
        Some("Файл модели дедупа повреждён — удалите и скачайте заново.".into())
    } else {
        None
    };

    LocalModelsOverview {
        server_installed: llm_dir::server_installed(),
        server_downloading,
        server_progress_pct,
        server_download_error: server_download_error.clone(),
        ready,
        dedup_ready,
        downloading,
        download_model_id,
        progress_pct: server_progress_pct,
        stage: if server_downloading {
            "server".into()
        } else if any_model_downloading {
            "model".into()
        } else {
            String::new()
        },
        error: server_download_error,
        runtime_error,
        dedup_runtime_error,
        device: settings.local_llm_device.clone(),
        gpu_layers: settings.local_llm_gpu_layers,
        active_ngl: settings.active_ngl(),
        active_model_id: active_id,
        active_dedup_model_id: active_dedup_id,
        models,
        disk_bytes: llm_dir::disk_usage_bytes(),
    }
}
