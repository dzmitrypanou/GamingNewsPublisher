use crate::services::{data_dir, watermark};
use serde::Serialize;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
pub struct WatermarkNaturalSize {
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub fn pick_watermark_file(app: AppHandle) -> Result<String, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Изображения", &["png", "webp", "jpg", "jpeg", "svg"])
        .set_title("Выберите файл водяного знака (PNG, JPG, SVG)")
        .blocking_pick_file();

    let picked = picked.ok_or_else(|| "Файл не выбран".to_string())?;
    let source = picked.into_path().map_err(|e| e.to_string())?;
    let ext = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();

    let data_dir = data_dir::resolve(&app).map_err(|e| e.to_string())?;
    let watermark_dir = data_dir::watermark_dir(&data_dir);
    let filename = format!("watermark.{ext}");
    let dest = watermark_dir.join(&filename);
    std::fs::copy(&source, &dest).map_err(|e| e.to_string())?;

    Ok(format!("local:watermark/{filename}"))
}

#[tauri::command]
pub fn get_watermark_natural_size(
    app: AppHandle,
    local_ref: String,
) -> Result<WatermarkNaturalSize, String> {
    let data_dir = data_dir::resolve(&app).map_err(|e| e.to_string())?;
    let path = crate::services::image_loader::resolve_local_image_path(&data_dir, &local_ref)
        .map_err(|e| e.to_string())?;
    let (width, height) = watermark::load_watermark_natural_size(Path::new(&path))
        .map_err(|e| e.to_string())?;
    Ok(WatermarkNaturalSize { width, height })
}
