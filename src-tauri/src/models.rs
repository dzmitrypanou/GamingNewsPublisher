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
    pub ai_prompt_template: String,
    pub auto_fetch: bool,
    pub fetch_interval_minutes: u32,
    pub fetch_items_per_source: u32,
    pub auto_publish: bool,
    pub auto_publish_interval_minutes: u32,
    pub auto_publish_jitter_seconds_min: u32,
    pub auto_publish_jitter_seconds_max: u32,
    pub auto_ai_process: bool,
    pub auto_approve: bool,
    pub ai_duplicate_check: bool,
    pub post_language: String,
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
            ai_prompt_template: DEFAULT_PROMPT.to_string(),
            auto_fetch: true,
            fetch_interval_minutes: 30,
            fetch_items_per_source: 10,
            auto_publish: false,
            auto_publish_interval_minutes: 60,
            auto_publish_jitter_seconds_min: 0,
            auto_publish_jitter_seconds_max: 60,
            auto_ai_process: true,
            auto_approve: true,
            ai_duplicate_check: false,
            post_language: "ru".to_string(),
        }
    }
}

pub const DEFAULT_PROMPT: &str = r##"Переведи игровую новость на {language} и перепиши для соцсетей VK и Telegram.
Все поля ответа строго на {language}.
Формат ответа JSON:
{
  "title": "короткий цепляющий заголовок (до 80 символов)",
  "text": "2-4 предложения, понятно и без воды (до 500 символов)",
  "hashtags": ["#игры", "#название_игры"]
}
Исходные данные: {title}, {description}, категория: {category}"##;

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
    pub last_fetch_errors: Vec<String>,
    pub auto_publish_enabled: bool,
    pub auto_publish_interval_minutes: u32,
    pub auto_publish_jitter_seconds_min: u32,
    pub auto_publish_jitter_seconds_max: u32,
    pub queue_size: i64,
    pub next_post: Option<QueuePostPreview>,
    pub next_publish_at: Option<String>,
    pub scheduled_delay_seconds: u64,
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
    pub processed_posts: i64,
    pub skipped_duplicates: i64,
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

#[derive(Debug, Clone, Deserialize)]
pub struct AiResponse {
    pub title: String,
    pub text: String,
    pub hashtags: Vec<String>,
}
