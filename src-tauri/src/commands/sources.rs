use crate::models::{PresetSource, RssPreviewItem, Source};
use crate::services::{data_dir, image_processor, rss_fetcher, settings_store};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_sources(state: State<'_, std::sync::Arc<AppState>>) -> Result<Vec<Source>, String> {
    state.db.get_sources().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_source(
    state: State<'_, std::sync::Arc<AppState>>,
    url: String,
    name: String,
    category_id: Option<i64>,
) -> Result<Source, String> {
    state
        .db
        .add_source(&url, &name, category_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_source(state: State<'_, std::sync::Arc<AppState>>, source: Source) -> Result<(), String> {
    state.db.update_source(&source).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_source(state: State<'_, std::sync::Arc<AppState>>, id: i64) -> Result<(), String> {
    state.db.delete_source(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_preset_sources() -> Vec<PresetSource> {
    rss_fetcher::get_preset_sources()
}

#[tauri::command]
pub fn add_preset_sources(state: State<'_, std::sync::Arc<AppState>>, urls: Vec<String>) -> Result<i64, String> {
    let presets = rss_fetcher::get_preset_sources();
    let mut added = 0i64;

    for url in urls {
        if state.db.source_exists(&url).unwrap_or(false) {
            continue;
        }
        let preset = presets.iter().find(|p| p.url == url);
        let (name, category_id) = if let Some(p) = preset {
            let cat = state
                .db
                .get_category_by_name(&p.category_name)
                .map_err(|e| e.to_string())?;
            (p.name.clone(), cat.map(|c| c.id))
        } else {
            (url.clone(), None)
        };

        if state.db.add_source(&url, &name, category_id).is_ok() {
            added += 1;
        }
    }

    Ok(added)
}

#[tauri::command]
pub async fn preview_source(
    state: State<'_, std::sync::Arc<AppState>>,
    url: String,
) -> Result<Vec<RssPreviewItem>, String> {
    let data_dir = data_dir::resolve(&state.app_handle).map_err(|e| e.to_string())?;
    let settings = settings_store::load_settings(&state.app_handle).map_err(|e| e.to_string())?;
    let image_options = image_processor::PostImageOptions::from_settings(&settings);
    let items = rss_fetcher::fetch_rss_items(&state.http_client(), &url, 3)
        .await
        .map_err(|e| e.to_string())?;

    let mut preview = Vec::new();
    for item in items {
        let image_url = image_processor::resolve_post_image(
            &state.http_client(),
            &data_dir,
            &item.link,
            &url,
            &item.title,
            item.image_url.as_deref(),
            image_options.clone(),
        )
        .await;

        preview.push(RssPreviewItem {
            title: item.title,
            description: item.description,
            link: item.link,
            image_url,
            pub_date: item.pub_date,
        });
    }

    Ok(preview)
}
