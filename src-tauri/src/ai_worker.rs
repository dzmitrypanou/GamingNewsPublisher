use crate::models::AppSettings;
use crate::services::{ai, deepseek, post_text, settings_store};
use crate::AppState;
use std::sync::Arc;
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};

const IDLE_POLL_SECS: u64 = 2;
const BATCH_SIZE: i64 = 10;

pub struct AiWorkerRuntime {
    notify: Notify,
    active_tasks: std::sync::atomic::AtomicUsize,
}

impl AiWorkerRuntime {
    pub fn new() -> Self {
        Self {
            notify: Notify::new(),
            active_tasks: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn wake(&self) {
        self.notify.notify_waiters();
    }

    pub fn active_count(&self) -> usize {
        self.active_tasks
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub fn start_ai_worker(state: Arc<AppState>, runtime: Arc<AiWorkerRuntime>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let settings = match settings_store::load_settings(&state.app_handle) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("AI worker settings: {}", e);
                    wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                    continue;
                }
            };

            if !settings.auto_ai_process
                || !ai::ai_is_configured_for_generation(&settings, &state.local_llm, &state.local_embed)
            {
                wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                continue;
            }

            if settings.generation_uses_local() {
                if state.local_llm.is_files_ready(&settings) && !state.local_llm.is_server_running() {
                    if let Err(e) = state.local_llm.start(&settings).await {
                        eprintln!("AI worker local LLM start: {}", e);
                        wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                        continue;
                    }
                }
                if !ai::ai_is_available_for_generation(&settings, &state.local_llm, &state.local_embed) {
                    wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                    continue;
                }
            } else if settings.deepseek_api_key.is_empty() {
                wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                continue;
            }

            let ai_concurrency = if settings.generation_uses_local() {
                1
            } else {
                settings.ai_process_concurrency.clamp(1, 10) as usize
            };
            let semaphore = Arc::new(Semaphore::new(ai_concurrency));

            let posts = match state.db.get_posts_by_status("new", BATCH_SIZE) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("AI worker db: {}", e);
                    wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                    continue;
                }
            };

            if posts.is_empty() {
                wait_or_notify(&runtime.notify, IDLE_POLL_SECS).await;
                continue;
            }

            let mut tasks = JoinSet::new();
            for post in posts {
                if !state.db.claim_post_for_ai(post.id) {
                    continue;
                }

                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        let _ = state.db.release_post_ai_claim(post.id);
                        break;
                    }
                };

                let state = state.clone();
                let settings = settings.clone();
                let runtime = runtime.clone();
                runtime
                    .active_tasks
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                tasks.spawn(async move {
                    let _permit = permit;
                    process_one_post(&state, post.id, &settings).await;
                    runtime
                        .active_tasks
                        .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                });
            }

            while tasks.join_next().await.is_some() {}
        }
    });
}

async fn wait_or_notify(notify: &Notify, secs: u64) {
    tokio::select! {
        _ = sleep(Duration::from_secs(secs)) => {}
        _ = notify.notified() => {}
    }
}

async fn process_one_post(state: &AppState, post_id: i64, settings: &AppSettings) {
    let post = match state.db.get_post(post_id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("AI worker get_post {}: {}", post_id, e);
            let _ = state.db.release_post_ai_claim(post_id);
            return;
        }
    };

    let category_name = post.category_name.as_deref().unwrap_or("Игры");

    match deepseek::process_news(
        &state.http_client(),
        settings,
        &state.local_llm,
        &post.raw_title,
        &post.raw_description,
        category_name,
        &post.source_url,
    )
    .await
    {
        Ok(ai_result) => {
            let hashtags = deepseek::format_hashtags(&ai_result.hashtags);
            let title = post_text::strip_links_single_line(&ai_result.title);
            let text = post_text::format_post_text(&ai_result.text);
            if let Err(e) = state.db.update_post_ai(
                post_id,
                &title,
                &text,
                &hashtags,
                settings.auto_approve,
            ) {
                eprintln!("AI worker update_post_ai {}: {}", post_id, e);
                let _ = state.db.release_post_ai_claim(post_id);
            }
        }
        Err(e) => {
            eprintln!("AI worker process_news {}: {}", post_id, e);
            let _ = state.db.release_post_ai_claim(post_id);
        }
    }
}
