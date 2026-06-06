use crate::models::Category;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_categories(state: State<'_, std::sync::Arc<AppState>>) -> Result<Vec<Category>, String> {
    state.db.get_categories().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_category(state: State<'_, std::sync::Arc<AppState>>, category: Category) -> Result<(), String> {
    state.db.update_category(&category).map_err(|e| e.to_string())
}
