use crate::models::Post;
use crate::services::{deepseek, post_text, settings_store};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn process_post_with_ai(
    state: State<'_, std::sync::Arc<AppState>>,
    id: i64,
) -> Result<Post, String> {
    let post = state.db.get_post(id).map_err(|e| e.to_string())?;
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;

    let category_name = post
        .category_name
        .as_deref()
        .unwrap_or("Игры");

    let ai_result = deepseek::process_news(
        &state.http_client(),
        &settings,
        &post.raw_title,
        &post.raw_description,
        category_name,
    )
    .await
    .map_err(|e| e.to_string())?;

    let hashtags = deepseek::format_hashtags(&ai_result.hashtags);
    let title = post_text::strip_links_single_line(&ai_result.title);
    let text = post_text::format_post_text(&ai_result.text);

    state
        .db
        .update_post_ai(
            id,
            &title,
            &text,
            &hashtags,
            settings.auto_approve,
        )
        .map_err(|e| e.to_string())?;

    state.db.get_post(id).map_err(|e| e.to_string())
}
