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
    pub fetch_interval_minutes: u32,
    pub auto_publish: bool,
    pub auto_ai_process: bool,
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
            fetch_interval_minutes: 30,
            auto_publish: false,
            auto_ai_process: true,
            post_language: "ru".to_string(),
        }
    }
}

pub const DEFAULT_PROMPT: &str = r##"Перепиши игровую новость для соцсетей VK и Telegram.
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
pub struct FetchResult {
    pub new_posts: i64,
    pub processed_posts: i64,
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
