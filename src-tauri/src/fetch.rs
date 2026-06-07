use crate::models::FetchResult;
use crate::services::{data_dir, deepseek, image_processor, post_text, rss_fetcher, settings_store};
use crate::AppState;
use anyhow::Result;

const AI_DUPLICATE_CHECK_LIMIT: i64 = 50;

pub async fn do_fetch(state: &AppState) -> Result<FetchResult> {
    if !state.fetch_runtime.try_begin() {
        anyhow::bail!("Сбор новостей уже выполняется");
    }

    let result = do_fetch_inner(state).await;
    let snapshot = match &result {
        Ok(r) => r.clone(),
        Err(e) => FetchResult {
            scanned_items: 0,
            new_posts: 0,
            processed_posts: 0,
            skipped_duplicates: 0,
            errors: vec![e.to_string()],
        },
    };
    state.fetch_runtime.finish(snapshot);
    result
}

async fn do_fetch_inner(state: &AppState) -> Result<FetchResult> {
    let settings = settings_store::load_settings(&state.app_handle)?;
    let sources = state.db.get_sources()?;
    let mut scanned_items = 0i64;
    let mut new_posts = 0i64;
    let mut processed_posts = 0i64;
    let mut skipped_duplicates = 0i64;
    let mut errors = Vec::new();
    let items_per_source = settings.fetch_items_per_source.clamp(1, 50) as usize;
    let ai_duplicate_enabled =
        settings.ai_duplicate_check && !settings.deepseek_api_key.is_empty();
    let app_data_dir = data_dir::resolve(&state.app_handle)?;
    let image_options = image_processor::PostImageOptions::from_settings(&settings);

    for mut source in sources {
        if !source.enabled {
            continue;
        }

        match rss_fetcher::fetch_rss_items(&state.http_client(), &source.url, items_per_source).await
        {
            Ok(items) => {
                scanned_items += items.len() as i64;
                for item in items {
                    if state.db.is_url_seen(&item.link).unwrap_or(false) {
                        continue;
                    }

                    if ai_duplicate_enabled {
                        match state.db.get_recent_posts(AI_DUPLICATE_CHECK_LIMIT) {
                            Ok(recent_posts) => {
                                match deepseek::find_ai_duplicate_among_posts(
                                    &state.http_client(),
                                    &settings,
                                    &item.title,
                                    &item.description,
                                    &recent_posts,
                                )
                                .await
                                {
                                    Ok(Some(dup)) => {
                                        let _ = state.db.record_ai_duplicate(
                                            &item.link,
                                            &item.title,
                                            &item.description,
                                            Some(dup.kept_post_id),
                                            Some(&dup.kept_title),
                                            &dup.analysis,
                                        );
                                        let _ =
                                            state.db.record_parsed_item(&item.link, &item.title);
                                        skipped_duplicates += 1;
                                        continue;
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        errors.push(format!(
                                            "AI дубль '{}': {}",
                                            item.title, e
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                errors.push(format!("DB: {}", e));
                            }
                        }
                    }

                    let image_url = image_processor::resolve_post_image(
                        &state.http_client(),
                        &app_data_dir,
                        &item.link,
                        &source.url,
                        &item.title,
                        item.image_url.as_deref(),
                        image_options.clone(),
                    )
                    .await;

                    match state.db.insert_post(
                        &item.link,
                        &item.title,
                        &item.description,
                        image_url.as_deref(),
                        source.category_id,
                    ) {
                        Ok(post_id) => {
                            new_posts += 1;
                            let mut ai_processed = false;

                            if settings.auto_ai_process && !settings.deepseek_api_key.is_empty() {
                                if let Ok(post) = state.db.get_post(post_id) {
                                    let category_name = post
                                        .category_name
                                        .as_deref()
                                        .unwrap_or("Игры");

                                    match deepseek::process_news(
                                        &state.http_client(),
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
                                            let title =
                                                post_text::strip_links_single_line(&ai_result.title);
                                            let text =
                                                post_text::format_post_text(&ai_result.text);
                                            let _ = state.db.update_post_ai(
                                                post_id,
                                                &title,
                                                &text,
                                                &hashtags,
                                                settings.auto_approve,
                                            );
                                            processed_posts += 1;
                                            ai_processed = true;
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

                            if settings.auto_approve && !ai_processed {
                                let _ = state.db.approve_post(post_id);
                            }
                        }
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
        scanned_items,
        new_posts,
        processed_posts,
        skipped_duplicates,
        errors,
    })
}
