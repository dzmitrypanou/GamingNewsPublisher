use crate::models::{FetchResult, Source};
use crate::services::rss_fetcher::RssItem;
use crate::services::{ai, data_dir, dedup_pipeline, image_processor, settings_store};
use crate::AppState;
use anyhow::Result;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{Duration, timeout};

const CANCEL_POLL_MS: u64 = 200;

pub(crate) struct FetchCounters {
    scanned_items: AtomicI64,
    new_posts: AtomicI64,
    ai_queued: AtomicI64,
    skipped_seen: AtomicI64,
    skipped_existing: AtomicI64,
    skipped_duplicates: AtomicI64,
    dedup_eligible: AtomicI64,
    dedup_checked: AtomicI64,
    errors: Mutex<Vec<String>>,
}

impl FetchCounters {
    pub(crate) fn new() -> Self {
        Self {
            scanned_items: AtomicI64::new(0),
            new_posts: AtomicI64::new(0),
            ai_queued: AtomicI64::new(0),
            skipped_seen: AtomicI64::new(0),
            skipped_existing: AtomicI64::new(0),
            skipped_duplicates: AtomicI64::new(0),
            dedup_eligible: AtomicI64::new(0),
            dedup_checked: AtomicI64::new(0),
            errors: Mutex::new(Vec::new()),
        }
    }

    fn push_error(&self, msg: String) {
        if let Ok(mut errors) = self.errors.lock() {
            errors.push(msg);
        }
    }

    pub(crate) fn snapshot(&self) -> FetchResult {
        FetchResult {
            scanned_items: self.scanned_items.load(Ordering::Relaxed),
            new_posts: self.new_posts.load(Ordering::Relaxed),
            ai_queued: self.ai_queued.load(Ordering::Relaxed),
            skipped_seen: self.skipped_seen.load(Ordering::Relaxed),
            skipped_existing: self.skipped_existing.load(Ordering::Relaxed),
            skipped_duplicates: self.skipped_duplicates.load(Ordering::Relaxed),
            dedup_checked: self.dedup_checked.load(Ordering::Relaxed),
            dedup_eligible: self.dedup_eligible.load(Ordering::Relaxed),
            errors: self.errors.lock().map(|e| e.clone()).unwrap_or_default(),
        }
    }
}

pub async fn do_fetch(state: Arc<AppState>) -> Result<FetchResult> {
    if !state.fetch_runtime.try_begin() {
        anyhow::bail!("Сбор новостей уже выполняется");
    }

    let result = do_fetch_inner(state.clone()).await;
    let snapshot = match &result {
        Ok(r) => r.clone(),
        Err(e) => FetchResult {
            scanned_items: 0,
            new_posts: 0,
            ai_queued: 0,
            skipped_seen: 0,
            skipped_existing: 0,
            skipped_duplicates: 0,
            dedup_checked: 0,
            dedup_eligible: 0,
            errors: vec![e.to_string()],
        },
    };
    state.fetch_runtime.finish(snapshot);
    result
}

async fn do_fetch_inner(state: Arc<AppState>) -> Result<FetchResult> {
    let settings = settings_store::load_settings(&state.app_handle)?;
    let sources = state.db.get_sources()?;
    let enabled_sources: Vec<Source> = sources.into_iter().filter(|s| s.enabled).collect();

    if enabled_sources.is_empty() {
        state.db.set_last_fetch_at()?;
        return Ok(FetchResult {
            scanned_items: 0,
            new_posts: 0,
            ai_queued: 0,
            skipped_seen: 0,
            skipped_existing: 0,
            skipped_duplicates: 0,
            dedup_checked: 0,
            dedup_eligible: 0,
            errors: vec![],
        });
    }

    let items_per_source = settings.fetch_items_per_source.clamp(1, 50) as usize;
    let ai_duplicate_enabled = settings.ai_duplicate_check
        && ai::ai_is_configured_for_duplicate(&settings, &state.local_llm, &state.local_embed);
    let auto_ai = settings.auto_ai_process
        && ai::ai_is_configured_for_generation(&settings, &state.local_llm, &state.local_embed);
    let app_data_dir = data_dir::resolve(&state.app_handle)?;
    let image_options = image_processor::PostImageOptions::from_settings(&settings);
    let counters = Arc::new(FetchCounters::new());
    state.fetch_runtime.set_active_counters(counters.clone());

    if ai_duplicate_enabled && settings.duplicate_uses_local() {
        if let Err(e) = state.local_llm.ensure_running(&settings).await {
            counters.push_error(format!("LLM для дублей: {e}"));
        }
    }

    let source_sem = Arc::new(Semaphore::new(
        settings.fetch_sources_concurrency.clamp(1, 20) as usize,
    ));
    let item_sem = Arc::new(Semaphore::new(
        settings.fetch_items_concurrency.clamp(1, 16) as usize,
    ));

    let mut source_tasks = JoinSet::new();
    for source in enabled_sources {
        if state.fetch_runtime.is_cancel_requested() {
            break;
        }
        let state = state.clone();
        let counters = counters.clone();
        let settings = settings.clone();
        let image_options = image_options.clone();
        let app_data_dir = app_data_dir.clone();
        let source_sem = source_sem.clone();
        let item_sem = item_sem.clone();

        source_tasks.spawn(async move {
            if state.fetch_runtime.is_cancel_requested() {
                return;
            }

            let _source_permit = match source_sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };

            if state.fetch_runtime.is_cancel_requested() {
                return;
            }

            let fetch_result = crate::services::rss_fetcher::fetch_rss_items(
                &state.http_client(),
                &source.url,
                items_per_source,
            )
            .await;

            let items = match fetch_result {
                Ok(items) => items,
                Err(e) => {
                    counters.push_error(format!("{}: {}", source.name, e));
                    return;
                }
            };

            counters
                .scanned_items
                .fetch_add(items.len() as i64, Ordering::Relaxed);

            let mut item_tasks = JoinSet::new();
            for item in items {
                if state.fetch_runtime.is_cancel_requested() {
                    break;
                }
                let state = state.clone();
                let counters = counters.clone();
                let settings = settings.clone();
                let image_options = image_options.clone();
                let app_data_dir = app_data_dir.clone();
                let item_sem = item_sem.clone();
                let source = source.clone();

                item_tasks.spawn(async move {
                    let _item_permit = match item_sem.acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => return,
                    };

                    process_item(
                        state,
                        &counters,
                        &settings,
                        &source,
                        item,
                        ai_duplicate_enabled,
                        auto_ai,
                        &app_data_dir,
                        image_options,
                    )
                    .await;
                });
            }

            join_set_until_done_or_cancel(&mut item_tasks, || {
                state.fetch_runtime.is_cancel_requested()
            })
            .await;

            if state.fetch_runtime.is_cancel_requested() {
                return;
            }

            let mut updated_source = source;
            updated_source.last_fetched_at = Some(chrono::Utc::now().to_rfc3339());
            if let Err(e) = state.db.update_source(&updated_source) {
                counters.push_error(format!("DB update source {}: {}", updated_source.name, e));
            }
        });
    }

    join_set_until_done_or_cancel(&mut source_tasks, || {
        state.fetch_runtime.is_cancel_requested()
    })
    .await;

    state.db.set_last_fetch_at()?;

    if auto_ai {
        state.ai_worker.wake();
    }

    Ok(counters.snapshot())
}

async fn join_set_until_done_or_cancel<T: Send + 'static>(
    tasks: &mut JoinSet<T>,
    is_cancelled: impl Fn() -> bool,
) {
    loop {
        if is_cancelled() {
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            return;
        }
        match timeout(
            Duration::from_millis(CANCEL_POLL_MS),
            tasks.join_next(),
        )
        .await
        {
            Ok(Some(_)) => continue,
            Ok(None) => return,
            Err(_) => continue,
        }
    }
}

async fn process_item(
    state: Arc<AppState>,
    counters: &FetchCounters,
    settings: &crate::models::AppSettings,
    source: &Source,
    item: RssItem,
    ai_duplicate_enabled: bool,
    auto_ai: bool,
    app_data_dir: &std::path::Path,
    image_options: image_processor::PostImageOptions,
) {
    if state.fetch_runtime.is_cancel_requested() {
        return;
    }

    if state.db.is_url_seen(&item.link).unwrap_or(false) {
        counters.skipped_seen.fetch_add(1, Ordering::Relaxed);
        return;
    }

    if ai_duplicate_enabled {
        counters.dedup_eligible.fetch_add(1, Ordering::Relaxed);
        if state.fetch_runtime.is_cancel_requested() {
            return;
        }
        match dedup_pipeline::check_duplicate(
            &state,
            settings,
            &item.title,
            &item.description,
            dedup_pipeline::DedupCheckOptions {
                exclude_post_id: None,
                status_filter: None,
                should_cancel: Some(Arc::new({
                    let state = state.clone();
                    move || state.fetch_runtime.is_cancel_requested()
                })),
            },
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
                let _ = state.db.record_parsed_item(&item.link, &item.title);
                counters
                    .skipped_duplicates
                    .fetch_add(1, Ordering::Relaxed);
                counters.dedup_checked.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Ok(None) => {
                counters.dedup_checked.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                counters.dedup_checked.fetch_add(1, Ordering::Relaxed);
                counters.push_error(format!("AI дубль '{}': {}", item.title, e));
            }
        }
    }

    if state.fetch_runtime.is_cancel_requested() {
        return;
    }

    let image_url = image_processor::resolve_post_image(
        &state.http_client(),
        app_data_dir,
        &item.link,
        &source.url,
        &item.title,
        item.image_url.as_deref(),
        image_options,
    )
    .await;

    match state.db.insert_post_if_new(
        &item.link,
        &item.title,
        &item.description,
        image_url.as_deref(),
        source.category_id,
    ) {
        Ok(Some(post_id)) => {
            counters.new_posts.fetch_add(1, Ordering::Relaxed);
            if auto_ai {
                counters.ai_queued.fetch_add(1, Ordering::Relaxed);
            } else if settings.auto_approve {
                let _ = state.db.approve_post(post_id);
            }
        }
        Ok(None) => {
            counters.skipped_existing.fetch_add(1, Ordering::Relaxed);
        }
        Err(e) => {
            counters.push_error(format!("DB {}: {}", source.name, e));
        }
    }
}
