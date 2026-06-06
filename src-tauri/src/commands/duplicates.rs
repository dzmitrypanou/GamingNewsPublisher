use crate::models::DuplicatesOverview;
use crate::services::settings_store;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_duplicates_overview(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<DuplicatesOverview, String> {
    let settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let mut overview = state
        .db
        .get_duplicates_overview()
        .map_err(|e| e.to_string())?;
    overview.ai_duplicate_check_enabled =
        settings.ai_duplicate_check && !settings.deepseek_api_key.is_empty();
    Ok(overview)
}
