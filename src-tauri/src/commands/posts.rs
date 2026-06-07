use crate::models::{DashboardStats, Post, PublishLog, PublishResult, UnpublishResult};
use crate::services::post_text;
use crate::AppState;
use tauri::State;

fn sanitize_post(mut post: Post) -> Post {
    if let Some(title) = post.ai_title.take() {
        post.ai_title = Some(post_text::strip_links_single_line(&title));
    }
    if let Some(text) = post.ai_text.take() {
        post.ai_text = Some(post_text::format_post_text(&text));
    }
    post
}

#[tauri::command]
pub fn get_posts(state: State<'_, std::sync::Arc<AppState>>, status: Option<String>) -> Result<Vec<Post>, String> {
    state
        .db
        .get_posts(status.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_post(state: State<'_, std::sync::Arc<AppState>>, id: i64) -> Result<Post, String> {
    state
        .db
        .get_post(id)
        .map(sanitize_post)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_post(state: State<'_, std::sync::Arc<AppState>>, post: Post) -> Result<(), String> {
    state
        .db
        .update_post(&sanitize_post(post))
        .map_err(|e| e.to_string())
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
pub fn get_recent_published_posts(
    state: State<'_, std::sync::Arc<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<Post>, String> {
    state
        .db
        .get_recent_published_posts(limit.unwrap_or(5))
        .map_err(|e| e.to_string())
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

#[tauri::command]
pub async fn unpublish_post(
    state: State<'_, std::sync::Arc<AppState>>,
    id: i64,
) -> Result<UnpublishResult, String> {
    crate::publish::do_unpublish(&state, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_queue_posts(state: State<'_, std::sync::Arc<AppState>>) -> Result<i64, String> {
    state.db.delete_queue_posts().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_all_data(state: State<'_, std::sync::Arc<AppState>>) -> Result<(), String> {
    state.db.reset_all_data().map_err(|e| e.to_string())?;
    if let Ok(data_dir) = crate::services::data_dir::resolve(&state.app_handle) {
        let _ = crate::services::image_loader::clear_images_dir(&data_dir);
    }
    Ok(())
}
