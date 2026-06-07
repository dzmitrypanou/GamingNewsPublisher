use crate::services::{data_dir, image_loader};
use crate::AppState;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use tauri::State;

#[tauri::command]
pub fn resolve_local_image_path(
    state: State<'_, std::sync::Arc<AppState>>,
    local_ref: String,
) -> Result<String, String> {
    let data_dir = data_dir::resolve(&state.app_handle).map_err(|e| e.to_string())?;
    let path = image_loader::resolve_local_image_path(&data_dir, &local_ref).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn read_local_image_data_url(
    state: State<'_, std::sync::Arc<AppState>>,
    local_ref: String,
) -> Result<String, String> {
    let data_dir = data_dir::resolve(&state.app_handle).map_err(|e| e.to_string())?;
    let path = image_loader::resolve_local_image_path(&data_dir, &local_ref).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mime = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .map(|ext| match ext.as_str() {
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => "image/jpeg",
        })
        .unwrap_or("image/jpeg");
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}
