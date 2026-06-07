use crate::models::Post;
use crate::services::{ai, deepseek, post_text, settings_store};
use crate::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn process_post_with_ai(
    state: State<'_, Arc<AppState>>,
    id: i64,
) -> Result<Post, String> {
    let post = state.db.get_post(id).map_err(|e| e.to_string())?;
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;

    if settings.generation_uses_local()
        && state.local_llm.is_files_ready(&settings)
        && !state.local_llm.is_server_running()
    {
        state.local_llm.start(&settings).await.map_err(|e| e.to_string())?;
    }

    if !ai::ai_is_available_for_generation(&settings, &state.local_llm, &state.local_embed) {
        return Err("AI недоступен для генерации: укажите API ключ или загрузите локальную модель".to_string());
    }

    let category_name = post
        .category_name
        .as_deref()
        .unwrap_or("Игры");

    let ai_result = deepseek::process_news(
        &state.http_client(),
        &settings,
        &state.local_llm,
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
