export interface AppSettings {
  vk_token: string;
  vk_group_id: string;
  telegram_bot_token: string;
  telegram_channel_id: string;
  deepseek_api_key: string;
  deepseek_model: string;
  ai_prompt_template: string;
  fetch_interval_minutes: number;
  auto_publish: boolean;
  auto_ai_process: boolean;
  post_language: string;
}

export interface Category {
  id: number;
  name: string;
  hashtags: string;
  keywords: string;
  enabled: boolean;
}

export interface Source {
  id: number;
  url: string;
  name: string;
  category_id: number | null;
  enabled: boolean;
  last_fetched_at: string | null;
}

export interface Post {
  id: number;
  source_url: string;
  raw_title: string;
  raw_description: string;
  raw_image_url: string | null;
  ai_title: string | null;
  ai_text: string | null;
  ai_hashtags: string | null;
  category_id: number | null;
  category_name: string | null;
  status: PostStatus;
  vk_post_id: string | null;
  telegram_message_id: string | null;
  created_at: string;
  published_at: string | null;
  error_message: string | null;
}

export type PostStatus =
  | "new"
  | "ai_processed"
  | "approved"
  | "published"
  | "failed";

export interface PublishLog {
  id: number;
  post_id: number;
  platform: string;
  success: boolean;
  response: string;
  created_at: string;
}

export interface DashboardStats {
  posts_today: number;
  posts_pending: number;
  posts_published: number;
  sources_active: number;
  last_fetch_at: string | null;
}

export interface ApiTestResult {
  success: boolean;
  message: string;
}

export interface FetchResult {
  new_posts: number;
  processed_posts: number;
  errors: string[];
}

export interface PublishResult {
  vk_success: boolean;
  vk_message: string;
  telegram_success: boolean;
  telegram_message: string;
}

export interface RssPreviewItem {
  title: string;
  description: string;
  link: string;
  image_url: string | null;
  pub_date: string | null;
}

export interface PresetSource {
  name: string;
  url: string;
  category_name: string;
  group: string;
}
