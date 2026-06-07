use crate::models::{AutomationStatus, FetchResult, QueuePostPreview};
use crate::services::settings_store;
use crate::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn fetch_news(state: State<'_, Arc<AppState>>) -> Result<FetchResult, String> {
    crate::fetch::do_fetch(state.inner().clone())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_fetch_news(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    Ok(state.fetch_runtime.request_cancel())
}

#[tauri::command]
pub fn get_automation_status(
    state: State<'_, Arc<AppState>>,
) -> Result<AutomationStatus, String> {
    let settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let stats = state.db.get_dashboard_stats().map_err(|e| e.to_string())?;
    let fetch_snapshot = if state.fetch_runtime.is_fetching() {
        state
            .fetch_runtime
            .live_snapshot()
            .or_else(|| state.fetch_runtime.last_result())
    } else {
        state.fetch_runtime.last_result()
    };

    let ai_queue_count = state
        .db
        .count_posts_by_status("new")
        .unwrap_or(0);
    let db_processing = state
        .db
        .count_posts_by_status("processing")
        .unwrap_or(0);
    let worker_active = state.ai_worker.active_count() as i64;
    let ai_processing_count = db_processing.max(worker_active);

    let next_post = state
        .db
        .get_next_publishable_post()
        .ok()
        .flatten()
        .map(|post| QueuePostPreview::from_post(&post));

    Ok(AutomationStatus {
        fetch_running: state.fetch_runtime.is_fetching(),
        auto_fetch_enabled: settings.auto_fetch,
        fetch_interval_minutes: settings.fetch_interval_minutes,
        last_fetch_at: stats.last_fetch_at,
        last_fetch_new_posts: fetch_snapshot.as_ref().map(|r| r.new_posts).unwrap_or(0),
        last_fetch_scanned_items: fetch_snapshot.as_ref().map(|r| r.scanned_items).unwrap_or(0),
        last_fetch_skipped_seen: fetch_snapshot.as_ref().map(|r| r.skipped_seen).unwrap_or(0),
        last_fetch_skipped_existing: fetch_snapshot
            .as_ref()
            .map(|r| r.skipped_existing)
            .unwrap_or(0),
        last_fetch_skipped_duplicates: fetch_snapshot
            .as_ref()
            .map(|r| r.skipped_duplicates)
            .unwrap_or(0),
        last_fetch_skipped_rejected: fetch_snapshot
            .as_ref()
            .map(|r| r.skipped_rejected)
            .unwrap_or(0),
        last_fetch_errors: fetch_snapshot
            .as_ref()
            .map(|r| r.errors.clone())
            .unwrap_or_default(),
        auto_publish_enabled: settings.auto_publish,
        auto_publish_interval_minutes: settings.auto_publish_interval_minutes,
        auto_publish_jitter_seconds_min: settings.auto_publish_jitter_seconds_min,
        auto_publish_jitter_seconds_max: settings.auto_publish_jitter_seconds_max,
        queue_size: stats.posts_pending,
        next_post,
        next_publish_at: state.auto_publish_runtime.next_publish_at(),
        scheduled_delay_seconds: state.auto_publish_runtime.scheduled_delay_secs(),
        ai_queue_count,
        ai_processing_count,
        ai_uses_local: settings.local_llm_needed(),
        ai_generation_uses_local: settings.generation_uses_local(),
        ai_duplicate_uses_local: settings.duplicate_uses_local(),
        ai_duplicate_check_enabled: settings.ai_duplicate_check,
        fetch_dedup_checked: fetch_snapshot.as_ref().map(|r| r.dedup_checked).unwrap_or(0),
        fetch_dedup_total: fetch_snapshot.as_ref().map(|r| r.dedup_eligible).unwrap_or(0),
    })
}
