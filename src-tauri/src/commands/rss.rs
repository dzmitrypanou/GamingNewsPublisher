use crate::models::{AutomationStatus, FetchResult, QueuePostPreview};
use crate::services::settings_store;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn fetch_news(state: State<'_, std::sync::Arc<AppState>>) -> Result<FetchResult, String> {
    crate::fetch::do_fetch(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_automation_status(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<AutomationStatus, String> {
    let settings =
        settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let stats = state.db.get_dashboard_stats().map_err(|e| e.to_string())?;
    let last_result = state.fetch_runtime.last_result();

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
        last_fetch_new_posts: last_result.as_ref().map(|r| r.new_posts).unwrap_or(0),
        last_fetch_errors: last_result
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
    })
}
