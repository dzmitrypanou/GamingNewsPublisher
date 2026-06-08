use crate::models::{
    DashboardStats, Post, PublishLog, PublishResult, RegenerateQueueImagesResult, UnpublishResult,
};
use crate::services::{
    ai, content_filter, data_dir, deepseek, image_processor, post_text, rss_fetcher, settings_store,
    web_context,
};
use crate::AppState;
use std::time::Duration;
use tauri::State;
use tokio::time::timeout;

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
    state.db.forget_post(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reprocess_post(
    state: State<'_, std::sync::Arc<AppState>>,
    id: i64,
) -> Result<Post, String> {
    let post = state.db.get_post(id).map_err(|e| e.to_string())?;
    if post.status == "published" {
        return Err("Нельзя перезагрузить опубликованный пост".to_string());
    }

    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let client = state.http_client();

    let fetched_title = web_context::fetch_page_title(&client, &post.source_url)
        .await
        .filter(|title| !title.trim().is_empty());
    let raw_title = fetched_title.as_deref().unwrap_or(&post.raw_title);

    let fetched_description = web_context::fetch_article_text(
        &client,
        &post.source_url,
        web_context::ArticleFetchMode::RssEnrich,
    )
    .await
    .map_err(|e| e.to_string())?;
    let raw_description = rss_fetcher::strip_boilerplate(&fetched_description);
    if raw_description.trim().is_empty() {
        return Err("Не удалось загрузить текст статьи по ссылке".to_string());
    }

    if content_filter::should_exclude_content(raw_title, &post.source_url, &raw_description, &[]) {
        state.db.forget_post(id).map_err(|e| e.to_string())?;
        return Err(
            "Пост не прошёл фильтр (например, подсказки к головоломкам) и удалён из базы. \
             При следующем сборе RSS он будет обработан заново."
                .to_string(),
        );
    }

    state
        .db
        .reset_post_raw(id, raw_title, &raw_description)
        .map_err(|e| e.to_string())?;

    if settings.auto_ai_process
        && ai::ai_is_available_for_generation(&settings, &state.local_llm, &state.local_embed)
    {
        if settings.generation_uses_local()
            && state.local_llm.is_files_ready(&settings)
            && !state.local_llm.is_server_running()
        {
            state
                .local_llm
                .start(&settings)
                .await
                .map_err(|e| e.to_string())?;
        }

        let category_name = post.category_name.as_deref().unwrap_or("Игры");
        let ai_result = deepseek::process_news(
            &client,
            &settings,
            &state.local_llm,
            raw_title,
            &raw_description,
            category_name,
            &post.source_url,
        )
        .await
        .map_err(|e| e.to_string())?;

        let hashtags = deepseek::format_hashtags(&ai_result.hashtags);
        let title = post_text::strip_links_single_line(&ai_result.title);
        let text = post_text::format_post_text(&ai_result.text);

        state
            .db
            .update_post_ai(id, &title, &text, &hashtags, settings.auto_approve)
            .map_err(|e| e.to_string())?;
    } else {
        state.ai_worker.wake();
    }

    state
        .db
        .get_post(id)
        .map(sanitize_post)
        .map_err(|e| e.to_string())
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
pub async fn regenerate_queue_images(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<RegenerateQueueImagesResult, String> {
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let data_dir = data_dir::resolve(&state.app_handle).map_err(|e| e.to_string())?;
    let image_options = image_processor::PostImageOptions::from_settings(&settings);
    let sources = state.db.get_sources().map_err(|e| e.to_string())?;
    let posts = state.db.get_queue_posts().map_err(|e| e.to_string())?;
    let client = state.http_client();

    let mut result = RegenerateQueueImagesResult {
        total: posts.len() as u32,
        updated: 0,
        failed: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for post in posts {
        let feed_url = image_processor::guess_feed_source_url(
            &sources,
            &post.source_url,
            post.category_id,
        );
        let rss_hint =
            image_processor::remote_image_hint(post.raw_image_url.as_deref());

        let resolved = timeout(
            Duration::from_secs(45),
            image_processor::resolve_post_image(
                &client,
                &data_dir,
                &post.source_url,
                &feed_url,
                &post.raw_title,
                rss_hint,
                image_options.clone(),
            ),
        )
        .await;

        match resolved {
            Ok(Some(new_url)) => {
                if post.raw_image_url.as_deref() == Some(new_url.as_str()) {
                    result.skipped += 1;
                    continue;
                }
                match state.db.update_post_image_url(post.id, Some(&new_url)) {
                    Ok(()) => result.updated += 1,
                    Err(e) => {
                        result.failed += 1;
                        if result.errors.len() < 20 {
                            result.errors.push(format!(
                                "Пост #{}: не удалось сохранить изображение: {}",
                                post.id, e
                            ));
                        }
                    }
                }
            }
            Ok(None) => result.skipped += 1,
            Err(_) => {
                result.failed += 1;
                if result.errors.len() < 20 {
                    result.errors.push(format!(
                        "Пост #{}: таймаут загрузки изображения",
                        post.id
                    ));
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn refresh_post_source(
    state: State<'_, std::sync::Arc<AppState>>,
    id: i64,
) -> Result<Post, String> {
    let post = state.db.get_post(id).map_err(|e| e.to_string())?;
    let enriched = web_context::enrich_rss_description(
        &state.http_client(),
        &post.source_url,
        &post.raw_description,
    )
    .await;

    if enriched.trim().is_empty() {
        return Err("Не удалось загрузить текст статьи по ссылке".to_string());
    }

    state
        .db
        .update_post_raw_description(id, &enriched)
        .map_err(|e| e.to_string())?;

    state
        .db
        .get_post(id)
        .map(sanitize_post)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_all_data(state: State<'_, std::sync::Arc<AppState>>) -> Result<(), String> {
    state.db.reset_all_data().map_err(|e| e.to_string())?;
    if let Ok(data_dir) = crate::services::data_dir::resolve(&state.app_handle) {
        let _ = crate::services::image_loader::clear_images_dir(&data_dir);
    }
    Ok(())
}
