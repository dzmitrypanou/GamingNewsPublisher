export interface AppSettings {
  vk_token: string;
  vk_group_id: string;
  telegram_bot_token: string;
  telegram_channel_id: string;
  deepseek_api_key: string;
  deepseek_model: string;
  ai_prompt_template: string;
  auto_fetch: boolean;
  fetch_interval_minutes: number;
  fetch_items_per_source: number;
  auto_publish: boolean;
  auto_publish_interval_minutes: number;
  auto_publish_jitter_seconds_min: number;
  auto_publish_jitter_seconds_max: number;
  auto_ai_process: boolean;
  auto_approve: boolean;
  ai_duplicate_check: boolean;
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

export interface QueuePostPreview {
  id: number;
  title: string;
  text: string;
  hashtags: string;
  image_url: string | null;
  status: string;
  category_name: string | null;
}

export interface AutomationStatus {
  fetch_running: boolean;
  auto_fetch_enabled: boolean;
  fetch_interval_minutes: number;
  last_fetch_at: string | null;
  last_fetch_new_posts: number;
  last_fetch_errors: string[];
  auto_publish_enabled: boolean;
  auto_publish_interval_minutes: number;
  auto_publish_jitter_seconds_min: number;
  auto_publish_jitter_seconds_max: number;
  queue_size: number;
  next_post: QueuePostPreview | null;
  next_publish_at: string | null;
  scheduled_delay_seconds: number;
}

export interface ApiTestResult {
  success: boolean;
  message: string;
}

export interface DuplicateRecord {
  id: number;
  duplicate_url: string;
  duplicate_title: string;
  duplicate_description: string;
  kept_post_id: number | null;
  kept_title: string | null;
  reason: string;
  created_at: string;
  ai_is_duplicate: boolean | null;
  ai_confidence: number | null;
  ai_explanation: string | null;
  ai_checked_at: string | null;
}

export interface DuplicateGroup {
  kept_post_id: number | null;
  kept_post: Post | null;
  duplicates: DuplicateRecord[];
}

export interface DuplicatesOverview {
  kept_count: number;
  duplicates_count: number;
  groups: DuplicateGroup[];
  standalone_posts: Post[];
  ai_duplicate_check_enabled: boolean;
}

export interface FetchResult {
  scanned_items: number;
  new_posts: number;
  processed_posts: number;
  skipped_duplicates: number;
  errors: string[];
}

export interface PublishResult {
  vk_success: boolean;
  vk_message: string;
  telegram_success: boolean;
  telegram_message: string;
}

export interface UnpublishResult {
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
