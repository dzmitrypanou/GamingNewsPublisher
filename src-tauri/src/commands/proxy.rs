use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn pick_proxy_file(app: AppHandle) -> Result<String, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Текстовые файлы", &["txt", "csv", "list"])
        .set_title("Выберите файл со списком прокси")
        .blocking_pick_file();

    let picked = picked.ok_or_else(|| "Файл не выбран".to_string())?;
    let path = picked.into_path().map_err(|e| e.to_string())?;
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_proxy_list(url: String) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Укажите ссылку на список прокси".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 GamingNewsPublisher/0.1",
        )
        .header("Accept", "text/plain, text/*, */*")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    response.text().await.map_err(|e| e.to_string())
}
