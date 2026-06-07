use crate::models::LocalModelsOverview;
use crate::services::{local_llm_download, local_llm_overview, settings_store};
use crate::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_local_models_overview(
    state: State<'_, Arc<AppState>>,
) -> Result<LocalModelsOverview, String> {
    Ok(local_llm_overview::build_overview_with_embed(
        &state.app_handle,
        &state.local_llm,
        Some(&state.local_embed),
        None,
    ))
}

#[tauri::command]
pub fn pause_local_model_download(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    local_llm_download::pause_model_download(
        &state.app_handle,
        &state.local_llm,
        &model_id,
    );
    Ok(())
}

#[tauri::command]
pub fn cancel_local_model_download(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    local_llm_download::cancel_model_download(
        &state.app_handle,
        &state.local_llm,
        &model_id,
    );
    Ok(())
}

#[tauri::command]
pub fn cancel_local_server_download(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    local_llm_download::cancel_server_download(&state.app_handle, &state.local_llm);
    Ok(())
}

#[tauri::command]
pub fn download_local_server(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    local_llm_download::start_server_download(
        state.app_handle.clone(),
        state.local_llm.clone(),
    );
    Ok(())
}

#[tauri::command]
pub fn download_local_model(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    local_llm_download::start_model_download(
        state.app_handle.clone(),
        state.local_llm.clone(),
        model_id,
    );
    Ok(())
}

#[tauri::command]
pub fn delete_local_model_partial(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    local_llm_download::delete_partial_download(
        &state.app_handle,
        &state.local_llm,
        &model_id,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_local_model(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let restart_settings = local_llm_download::delete_local_model(
        &state.app_handle,
        &state.local_llm,
        &settings,
        &model_id,
    )
    .map_err(|e| e.to_string())?;
    if settings.normalized_local_dedup_model_id() == model_id {
        state.local_embed.shutdown();
    }
    if let Some(new_settings) = restart_settings {
        if new_settings.local_generation_needed()
            && crate::services::llm_dir::files_ready(&new_settings.normalized_local_model_id())
        {
            state
                .local_llm
                .start(&new_settings)
                .await
                .map_err(|e| e.to_string())?;
        }
        if new_settings.local_embed_needed() {
            let dedup_id = new_settings.normalized_local_dedup_model_id();
            if state.local_embed.is_files_ready(&dedup_id) {
                state
                    .local_embed
                    .start(&new_settings, &dedup_id)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn set_local_model(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let mut settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    if crate::services::local_model_catalog::find(&model_id).is_none() {
        return Err("Unknown model".into());
    }
    let def = crate::services::local_model_catalog::find(&model_id).unwrap();
    if def.model_kind.uses_embeddings() {
        return Err("Это модель для проверки дублей — выберите её в блоке «Модель дедупа»".into());
    }
    if !crate::services::llm_dir::model_installed(&model_id) {
        return Err("Model not installed".into());
    }

    settings.local_model_id = model_id;
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;

    state.local_llm.shutdown();
    if settings.local_generation_needed() {
        state
            .local_llm
            .start(&settings)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn set_local_dedup_model(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    let mut settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let def = crate::services::local_model_catalog::find(&model_id)
        .ok_or_else(|| "Unknown model".to_string())?;
    if !crate::services::llm_dir::dedup_model_selectable(&model_id) {
        let reason = def
            .deprecated_reason
            .clone()
            .unwrap_or_else(|| "Модель не подходит для проверки дублей".into());
        return Err(reason);
    }
    if !crate::services::llm_dir::model_installed(&model_id) {
        return Err("Model not installed".into());
    }

    settings.local_dedup_model_id = model_id;
    settings_store::save_settings(&state.app_handle, &settings).map_err(|e| e.to_string())?;

    state.local_embed.shutdown();
    if settings.local_embed_needed() {
        let dedup_id = settings.normalized_local_dedup_model_id();
        state
            .local_embed
            .start(&settings, &dedup_id)
            .await
            .map_err(|e| e.to_string())?;
    }

    local_llm_download::emit_overview(&state.app_handle, &state.local_llm);
    Ok(())
}

#[tauri::command]
pub fn start_local_llm_download(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    if !crate::services::llm_dir::server_installed() {
        local_llm_download::start_server_download(
            state.app_handle.clone(),
            state.local_llm.clone(),
        );
    } else if !crate::services::llm_dir::model_installed(&settings.normalized_local_model_id()) {
        local_llm_download::start_model_download(
            state.app_handle.clone(),
            state.local_llm.clone(),
            settings.normalized_local_model_id(),
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn add_custom_local_model(
    state: State<'_, Arc<AppState>>,
    name: String,
    description: String,
    download_url: String,
) -> Result<(), String> {
    crate::services::custom_model_store::add(
        &state.http_client(),
        name,
        description,
        download_url,
    )
    .await
    .map_err(|e| e.to_string())?;
    local_llm_download::emit_overview(&state.app_handle, &state.local_llm);
    Ok(())
}

#[tauri::command]
pub fn remove_custom_local_model(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), String> {
    if !crate::services::custom_model_store::is_custom_model(&model_id) {
        return Err("Это не пользовательская модель".into());
    }
    local_llm_download::delete_partial_download(
        &state.app_handle,
        &state.local_llm,
        &model_id,
    )
    .ok();
    crate::services::custom_model_store::remove(&model_id).map_err(|e| e.to_string())?;
    local_llm_download::emit_overview(&state.app_handle, &state.local_llm);
    Ok(())
}

// Backward compat for old frontend calls
#[tauri::command]
pub fn get_local_llm_status(
    state: State<'_, Arc<AppState>>,
) -> Result<LocalModelsOverview, String> {
    get_local_models_overview(state)
}
