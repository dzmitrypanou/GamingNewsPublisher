use crate::models::{DashboardStats, Post, PublishLog, PublishResult};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_posts(state: State<'_, std::sync::Arc<AppState>>, status: Option<String>) -> Result<Vec<Post>, String> {
    state
        .db
        .get_posts(status.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_post(state: State<'_, std::sync::Arc<AppState>>, id: i64) -> Result<Post, String> {
    state.db.get_post(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_post(state: State<'_, std::sync::Arc<AppState>>, post: Post) -> Result<(), String> {
    state.db.update_post(&post).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_post(state: State<'_, std::sync::Arc<AppState>>, id: i64) -> Result<(), String> {
    state.db.delete_post(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_dashboard_stats(state: State<'_, std::sync::Arc<AppState>>) -> Result<DashboardStats, String> {
    state.db.get_dashboard_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_publish_history(state: State<'_, std::sync::Arc<AppState>>) -> Result<Vec<PublishLog>, String> {
    state.db.get_publish_history().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_published_posts(state: State<'_, std::sync::Arc<AppState>>) -> Result<Vec<Post>, String> {
    state.db.get_published_posts().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn publish_post(
    state: State<'_, std::sync::Arc<AppState>>,
    id: i64,
) -> Result<PublishResult, String> {
    crate::publish::do_publish(&state, id)
        .await
        .map_err(|e| e.to_string())
}
