use crate::models::BackupExportResult;
use crate::services::{backup, data_dir};
use crate::AppState;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn pick_backup_directory(app: AppHandle) -> Result<String, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Папка для автоматических бэкапов")
        .blocking_pick_folder();

    let picked = picked.ok_or_else(|| "Папка не выбрана".to_string())?;
    let path = picked.into_path().map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn export_backup_manual(state: State<'_, Arc<AppState>>, app: AppHandle) -> Result<BackupExportResult, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Сохранить бэкап")
        .set_file_name(&backup::default_backup_filename())
        .add_filter("Архив бэкапа", &["zip"])
        .blocking_save_file();

    let picked = picked.ok_or_else(|| "Сохранение отменено".to_string())?;
    let path = picked.into_path().map_err(|e| e.to_string())?;

    let data_dir = data_dir::resolve(&state.app_handle).map_err(|e| e.to_string())?;
    state.db.checkpoint_wal().map_err(|e| e.to_string())?;
    let result = backup::export_backup(&data_dir, &path).map_err(|e| e.to_string())?;
    let _ = state.db.set_last_backup_at();
    Ok(result)
}

#[tauri::command]
pub fn import_backup(state: State<'_, Arc<AppState>>, app: AppHandle) -> Result<(), String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Выберите файл бэкапа")
        .add_filter("Архив бэкапа", &["zip"])
        .blocking_pick_file();

    let picked = picked.ok_or_else(|| "Файл не выбран".to_string())?;
    let path = picked.into_path().map_err(|e| e.to_string())?;

    state.local_llm.shutdown();
    state.local_embed.shutdown();

    let data_dir = data_dir::resolve(&state.app_handle).map_err(|e| e.to_string())?;
    state.db.checkpoint_wal().map_err(|e| e.to_string())?;
    backup::import_backup(&data_dir, &path).map_err(|e| e.to_string())?;

    state.app_handle.restart();
}
