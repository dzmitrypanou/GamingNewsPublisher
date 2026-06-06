use crate::models::FetchResult;
use crate::services::{deepseek, rss_fetcher, settings_store};
use crate::AppState;
use anyhow::Result;

pub async fn do_fetch(state: &AppState) -> Result<FetchResult> {
    let settings = settings_store::load_settings(&state.app_handle)?;
    let sources = state.db.get_sources()?;
    let mut new_posts = 0i64;
    let mut processed_posts = 0i64;
    let mut errors = Vec::new();

    for mut source in sources {
        if !source.enabled {
            continue;
        }

        match rss_fetcher::fetch_rss_items(&state.http_client, &source.url, 10).await {
            Ok(items) => {
                for item in items {
                    let mut image_url = item.image_url.clone();

                    if image_url.is_none() {
                        image_url =
                            rss_fetcher::fetch_og_image(&state.http_client, &item.link).await;
                    }

                    match state.db.insert_post(
                        &item.link,
                        &item.title,
                        &item.description,
                        image_url.as_deref(),
                        source.category_id,
                    ) {
                        Ok(Some(post_id)) => {
                            new_posts += 1;

                            if settings.auto_ai_process && !settings.deepseek_api_key.is_empty() {
                                if let Ok(post) = state.db.get_post(post_id) {
                                    let category_name = post
                                        .category_name
                                        .as_deref()
                                        .unwrap_or("Игры");

                                    match deepseek::process_news(
                                        &state.http_client,
                                        &settings,
                                        &post.raw_title,
                                        &post.raw_description,
                                        category_name,
                                    )
                                    .await
                                    {
                                        Ok(ai_result) => {
                                            let hashtags =
                                                deepseek::format_hashtags(&ai_result.hashtags);
                                            let _ = state.db.update_post_ai(
                                                post_id,
                                                &ai_result.title,
                                                &ai_result.text,
                                                &hashtags,
                                            );
                                            processed_posts += 1;

                                            if settings.auto_publish {
                                                if let Err(e) =
                                                    crate::publish::do_publish(state, post_id).await
                                                {
                                                    errors.push(format!(
                                                        "Auto-publish {}: {}",
                                                        post_id, e
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            errors.push(format!(
                                                "AI для '{}': {}",
                                                item.title, e
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            errors.push(format!("DB {}: {}", source.name, e));
                        }
                    }
                }

                source.last_fetched_at = Some(chrono::Utc::now().to_rfc3339());
                let _ = state.db.update_source(&source);
            }
            Err(e) => {
                errors.push(format!("{}: {}", source.name, e));
            }
        }
    }

    state.db.set_last_fetch_at()?;

    Ok(FetchResult {
        new_posts,
        processed_posts,
        errors,
    })
}
