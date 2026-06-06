use crate::models::FetchResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn fetch_news(state: State<'_, std::sync::Arc<AppState>>) -> Result<FetchResult, String> {
    crate::fetch::do_fetch(&state).await.map_err(|e| e.to_string())
}
