use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppSettings {
    pub vk_token: String,
    pub vk_group_id: String,
    pub telegram_bot_token: String,
    pub telegram_channel_id: String,
    pub deepseek_api_key: String,
    pub deepseek_model: String,
    pub ai_provider: String,
    pub ai_generation_provider: String,
    pub ai_duplicate_provider: String,
    pub local_model_id: String,
    pub local_dedup_model_id: String,
    pub local_llm_device: String,
    pub local_llm_gpu_layers: u32,
    pub ai_prompt_template: String,
    pub auto_fetch: bool,
    pub fetch_interval_minutes: u32,
    pub fetch_items_per_source: u32,
    pub fetch_sources_concurrency: u32,
    pub fetch_items_concurrency: u32,
    pub ai_dedup_concurrency: u32,
    pub ai_process_concurrency: u32,
    pub auto_publish: bool,
    pub auto_publish_interval_minutes: u32,
    pub auto_publish_jitter_seconds_min: u32,
    pub auto_publish_jitter_seconds_max: u32,
    pub auto_ai_process: bool,
    pub auto_approve: bool,
    pub ai_duplicate_check: bool,
    pub post_language: String,
    pub proxy_enabled: bool,
    pub proxy_type: String,
    pub proxy_list: String,
    pub post_image_width: u32,
    pub post_image_height: u32,
    pub watermark_enabled: bool,
    pub watermark_image: String,
    pub watermark_opacity: u32,
    pub watermark_scale_percent: u32,
    pub watermark_position_mode: String,
    pub watermark_preset: String,
    pub watermark_margin_x: u32,
    pub watermark_margin_y: u32,
    pub watermark_x: u32,
    pub watermark_y: u32,
    pub watermark_size_mode: String,
    pub watermark_width_px: u32,
    pub watermark_height_px: u32,
    pub web_context_enabled: bool,
    pub web_search_provider: String,
    pub tavily_api_key: String,
    pub ai_duplicate_window_days: u32,
    pub ai_duplicate_check_limit: u32,
    pub ai_duplicate_llm_top_k: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            vk_token: String::new(),
            vk_group_id: String::new(),
            telegram_bot_token: String::new(),
            telegram_channel_id: String::new(),
            deepseek_api_key: String::new(),
            deepseek_model: "deepseek-chat".to_string(),
            ai_provider: "local".to_string(),
            ai_generation_provider: "local".to_string(),
            ai_duplicate_provider: "local".to_string(),
            local_model_id: local_model_catalog_id(),
            local_dedup_model_id: local_dedup_model_catalog_id(),
            local_llm_device: "gpu".to_string(),
            local_llm_gpu_layers: 28,
            ai_prompt_template: DEFAULT_PROMPT.to_string(),
            auto_fetch: true,
            fetch_interval_minutes: 30,
            fetch_items_per_source: 10,
            fetch_sources_concurrency: 6,
            fetch_items_concurrency: 4,
            ai_dedup_concurrency: 2,
            ai_process_concurrency: 3,
            auto_publish: false,
            auto_publish_interval_minutes: 60,
            auto_publish_jitter_seconds_min: 0,
            auto_publish_jitter_seconds_max: 60,
            auto_ai_process: true,
            auto_approve: true,
            ai_duplicate_check: true,
            post_language: "ru".to_string(),
            proxy_enabled: false,
            proxy_type: "http".to_string(),
            proxy_list: String::new(),
            post_image_width: 1280,
            post_image_height: 720,
            watermark_enabled: false,
            watermark_image: String::new(),
            watermark_opacity: 85,
            watermark_scale_percent: 18,
            watermark_position_mode: "preset".to_string(),
            watermark_preset: "bottom_right".to_string(),
            watermark_margin_x: 24,
            watermark_margin_y: 24,
            watermark_x: 0,
            watermark_y: 0,
            watermark_size_mode: "scale".to_string(),
            watermark_width_px: 0,
            watermark_height_px: 0,
            web_context_enabled: true,
            web_search_provider: "article_only".to_string(),
            tavily_api_key: String::new(),
            ai_duplicate_window_days: 30,
            ai_duplicate_check_limit: 200,
            ai_duplicate_llm_top_k: 50,
        }
    }
}

impl AppSettings {
    pub fn uses_local_ai(&self) -> bool {
        self.local_llm_needed()
    }

    pub fn generation_uses_local(&self) -> bool {
        self.ai_generation_provider == "local"
    }

    pub fn duplicate_uses_local(&self) -> bool {
        self.ai_duplicate_provider == "local"
    }

    pub fn generation_uses_cloud(&self) -> bool {
        self.ai_generation_provider == "cloud"
    }

    pub fn duplicate_uses_cloud(&self) -> bool {
        self.ai_duplicate_provider == "cloud"
    }

    pub fn local_llm_needed(&self) -> bool {
        self.local_generation_needed() || self.duplicate_uses_local()
    }

    pub fn normalized_local_model_id(&self) -> String {
        crate::services::local_model_catalog::normalize_model_id(&self.local_model_id).to_string()
    }

    pub fn normalized_local_dedup_model_id(&self) -> String {
        self.normalized_local_model_id()
    }

    pub fn duplicate_uses_embeddings(&self) -> bool {
        false
    }

    pub fn local_embed_needed(&self) -> bool {
        false
    }

    pub fn local_generation_needed(&self) -> bool {
        self.generation_uses_local()
    }

    pub fn effective_ai_model(&self) -> String {
        self.effective_generation_model()
    }

    pub fn effective_generation_model(&self) -> String {
        if self.generation_uses_local() {
            crate::services::local_model_catalog::find(&self.local_model_id)
                .map(|m| m.name.to_string())
                .unwrap_or_else(|| self.normalized_local_model_id())
        } else {
            self.deepseek_model.clone()
        }
    }

    pub fn effective_duplicate_model(&self) -> String {
        if self.duplicate_uses_local() {
            crate::services::local_model_catalog::find(&self.normalized_local_model_id())
                .map(|m| m.name.to_string())
                .unwrap_or_else(|| self.normalized_local_model_id())
        } else {
            self.deepseek_model.clone()
        }
    }

    pub fn active_ngl(&self) -> u32 {
        crate::services::local_model_catalog::resolve_ngl(
            &self.local_llm_device,
            self.local_llm_gpu_layers,
        )
    }
}

fn local_model_catalog_id() -> String {
    crate::services::local_model_catalog::default_model_id().to_string()
}

fn local_dedup_model_catalog_id() -> String {
    crate::services::local_model_catalog::default_dedup_model_id().to_string()
}

pub const DEFAULT_PROMPT: &str = r##"Переведи игровую новость на {language} и перепиши для соцсетей VK и Telegram.
Если исходный текст на другом языке — переведи. Если уже на {language} — перепиши живым языком для соцсетей.
Не выдумывай факты: опирайся только на {title}, {description} и дополнительный контекст ниже.
Все поля ответа строго на {language}.
Формат ответа JSON:
{
  "title": "короткий цепляющий заголовок (до 80 символов)",
  "text": "2-4 предложения в 1-2 абзаца, между абзацами пустая строка (\\n\\n), без ссылок (до 500 символов)",
  "hashtags": ["#игры", "#название_игры"]
}
Исходные данные: {title}, {description}, категория: {category}
{web_context}"##;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub hashtags: String,
    pub keywords: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: i64,
    pub url: String,
    pub name: String,
    pub category_id: Option<i64>,
    pub enabled: bool,
    pub last_fetched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: i64,
    pub source_url: String,
    pub raw_title: String,
    pub raw_description: String,
    pub raw_image_url: Option<String>,
    pub ai_title: Option<String>,
    pub ai_text: Option<String>,
    pub ai_hashtags: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub status: String,
    pub vk_post_id: Option<String>,
    pub telegram_message_id: Option<String>,
    pub created_at: String,
    pub published_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishLog {
    pub id: i64,
    pub post_id: i64,
    pub platform: String,
    pub success: bool,
    pub response: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuePostPreview {
    pub id: i64,
    pub title: String,
    pub text: String,
    pub hashtags: String,
    pub image_url: Option<String>,
    pub status: String,
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStatus {
    pub fetch_running: bool,
    pub auto_fetch_enabled: bool,
    pub fetch_interval_minutes: u32,
    pub last_fetch_at: Option<String>,
    pub last_fetch_new_posts: i64,
    pub last_fetch_scanned_items: i64,
    pub last_fetch_skipped_seen: i64,
    pub last_fetch_skipped_existing: i64,
    pub last_fetch_skipped_duplicates: i64,
    pub last_fetch_errors: Vec<String>,
    pub auto_publish_enabled: bool,
    pub auto_publish_interval_minutes: u32,
    pub auto_publish_jitter_seconds_min: u32,
    pub auto_publish_jitter_seconds_max: u32,
    pub queue_size: i64,
    pub next_post: Option<QueuePostPreview>,
    pub next_publish_at: Option<String>,
    pub scheduled_delay_seconds: u64,
    pub ai_queue_count: i64,
    pub ai_processing_count: i64,
    pub ai_uses_local: bool,
    pub ai_generation_uses_local: bool,
    pub ai_duplicate_uses_local: bool,
    pub ai_duplicate_check_enabled: bool,
    pub fetch_dedup_checked: i64,
    pub fetch_dedup_total: i64,
}

impl QueuePostPreview {
    pub fn from_post(post: &Post) -> Self {
        Self {
            id: post.id,
            title: post
                .ai_title
                .clone()
                .unwrap_or_else(|| post.raw_title.clone()),
            text: post
                .ai_text
                .clone()
                .unwrap_or_else(|| post.raw_description.clone()),
            hashtags: post.ai_hashtags.clone().unwrap_or_default(),
            image_url: post.raw_image_url.clone(),
            status: post.status.clone(),
            category_name: post.category_name.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub posts_today: i64,
    pub posts_pending: i64,
    pub posts_published: i64,
    pub sources_active: i64,
    pub last_fetch_at: Option<String>,
    pub duplicates_total: i64,
    pub posts_waiting_ai: i64,
    pub posts_processing_ai: i64,
    pub posts_ai_processed: i64,
    pub posts_approved: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTestResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateRecord {
    pub id: i64,
    pub duplicate_url: String,
    pub duplicate_title: String,
    pub duplicate_description: String,
    pub kept_post_id: Option<i64>,
    pub kept_title: Option<String>,
    pub reason: String,
    pub created_at: String,
    pub ai_is_duplicate: Option<bool>,
    pub ai_confidence: Option<u32>,
    pub ai_explanation: Option<String>,
    pub ai_checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub kept_post_id: Option<i64>,
    pub kept_post: Option<Post>,
    pub duplicates: Vec<DuplicateRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatesOverview {
    pub kept_count: i64,
    pub duplicates_count: i64,
    pub groups: Vec<DuplicateGroup>,
    pub standalone_posts: Vec<Post>,
    pub ai_duplicate_check_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateAiAnalysis {
    pub is_duplicate: bool,
    pub confidence: u32,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    pub scanned_items: i64,
    pub new_posts: i64,
    pub ai_queued: i64,
    pub skipped_seen: i64,
    pub skipped_existing: i64,
    pub skipped_duplicates: i64,
    pub dedup_checked: i64,
    pub dedup_eligible: i64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    pub vk_success: bool,
    pub vk_message: String,
    pub telegram_success: bool,
    pub telegram_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpublishResult {
    pub vk_success: bool,
    pub vk_message: String,
    pub telegram_success: bool,
    pub telegram_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssPreviewItem {
    pub title: String,
    pub description: String,
    pub link: String,
    pub image_url: Option<String>,
    pub pub_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSource {
    pub name: String,
    pub url: String,
    pub category_name: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_hint_bytes: u64,
    pub min_vram_gb: u8,
    pub layer_count_hint: u32,
    pub recommended: bool,
    pub deprecated_reason: Option<String>,
    pub installed: bool,
    pub install_invalid: bool,
    pub file_bytes: u64,
    pub is_active: bool,
    pub downloading: bool,
    pub has_partial_download: bool,
    pub progress_pct: f64,
    pub download_error: Option<String>,
    pub is_custom: bool,
    pub model_kind: String,
    pub is_active_dedup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelsOverview {
    pub server_installed: bool,
    pub server_downloading: bool,
    pub server_progress_pct: f64,
    pub server_download_error: Option<String>,
    pub ready: bool,
    pub dedup_ready: bool,
    pub downloading: bool,
    pub download_model_id: Option<String>,
    pub progress_pct: f64,
    pub stage: String,
    pub error: Option<String>,
    pub runtime_error: Option<String>,
    pub dedup_runtime_error: Option<String>,
    pub device: String,
    pub gpu_layers: u32,
    pub active_ngl: u32,
    pub active_model_id: String,
    pub active_dedup_model_id: String,
    pub models: Vec<LocalModelInfo>,
    pub disk_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLlmStatus {
    pub ready: bool,
    pub downloading: bool,
    pub progress_pct: f64,
    pub stage: String,
    pub error: Option<String>,
    pub server_installed: bool,
    pub model_installed: bool,
    pub disk_bytes: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiResponse {
    pub title: String,
    pub text: String,
    pub hashtags: Vec<String>,
}
