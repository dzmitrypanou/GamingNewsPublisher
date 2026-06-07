export interface AppSettings {
  vk_token: string;
  vk_group_id: string;
  telegram_bot_token: string;
  telegram_channel_id: string;
  deepseek_api_key: string;
  deepseek_model: string;
  ai_provider: "cloud" | "local";
  ai_generation_provider: "local" | "cloud" | "off";
  ai_duplicate_provider: "local" | "cloud" | "off";
  local_model_id: string;
  local_dedup_model_id: string;
  local_llm_device: "cpu" | "gpu" | "hybrid";
  local_llm_gpu_layers: number;
  ai_prompt_template: string;
  auto_fetch: boolean;
  fetch_interval_minutes: number;
  fetch_items_per_source: number;
  fetch_sources_concurrency: number;
  fetch_items_concurrency: number;
  ai_dedup_concurrency: number;
  ai_process_concurrency: number;
  auto_publish: boolean;
  auto_publish_interval_minutes: number;
  auto_publish_jitter_seconds_min: number;
  auto_publish_jitter_seconds_max: number;
  auto_ai_process: boolean;
  auto_approve: boolean;
  ai_duplicate_check: boolean;
  post_language: string;
  proxy_enabled: boolean;
  proxy_type: "http" | "https" | "socks5";
  proxy_list: string;
  post_image_width: number;
  post_image_height: number;
  watermark_enabled: boolean;
  watermark_image: string;
  watermark_opacity: number;
  watermark_scale_percent: number;
  watermark_position_mode: WatermarkPositionMode;
  watermark_preset: WatermarkPreset;
  watermark_margin_x: number;
  watermark_margin_y: number;
  watermark_x: number;
  watermark_y: number;
  watermark_size_mode: WatermarkSizeMode;
  watermark_width_px: number;
  watermark_height_px: number;
}

export type WatermarkSizeMode = "scale" | "custom";

export type WatermarkPositionMode = "preset" | "manual";

export type WatermarkPreset =
  | "top_left"
  | "top_center"
  | "top_right"
  | "center_left"
  | "center"
  | "center_right"
  | "bottom_left"
  | "bottom_center"
  | "bottom_right";

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
  | "processing"
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
  duplicates_total: number;
  posts_waiting_ai: number;
  posts_processing_ai: number;
  posts_ai_processed: number;
  posts_approved: number;
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
  last_fetch_scanned_items: number;
  last_fetch_skipped_seen: number;
  last_fetch_skipped_existing: number;
  last_fetch_skipped_duplicates: number;
  last_fetch_errors: string[];
  auto_publish_enabled: boolean;
  auto_publish_interval_minutes: number;
  auto_publish_jitter_seconds_min: number;
  auto_publish_jitter_seconds_max: number;
  queue_size: number;
  next_post: QueuePostPreview | null;
  next_publish_at: string | null;
  scheduled_delay_seconds: number;
  ai_queue_count: number;
  ai_processing_count: number;
  ai_uses_local: boolean;
  ai_generation_uses_local: boolean;
  ai_duplicate_uses_local: boolean;
  ai_duplicate_check_enabled: boolean;
  fetch_dedup_checked: number;
  fetch_dedup_total: number;
}

export interface LocalModelInfo {
  id: string;
  name: string;
  description: string;
  size_hint_bytes: number;
  min_vram_gb: number;
  layer_count_hint: number;
  recommended: boolean;
  deprecated_reason: string | null;
  installed: boolean;
  install_invalid: boolean;
  file_bytes: number;
  is_active: boolean;
  downloading: boolean;
  has_partial_download: boolean;
  progress_pct: number;
  download_error: string | null;
  is_custom: boolean;
  model_kind: "llm" | "encoder" | "nli";
  is_active_dedup: boolean;
}

export interface LocalModelsOverview {
  server_installed: boolean;
  server_downloading: boolean;
  server_progress_pct: number;
  server_download_error: string | null;
  ready: boolean;
  dedup_ready: boolean;
  downloading: boolean;
  download_model_id: string | null;
  progress_pct: number;
  stage: string;
  error: string | null;
  runtime_error: string | null;
  dedup_runtime_error: string | null;
  device: string;
  gpu_layers: number;
  active_ngl: number;
  active_model_id: string;
  active_dedup_model_id: string;
  models: LocalModelInfo[];
  disk_bytes: number;
}

/** @deprecated use LocalModelsOverview */
export type LocalLlmStatus = LocalModelsOverview;

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
  ai_queued: number;
  skipped_seen: number;
  skipped_existing: number;
  skipped_duplicates: number;
  dedup_checked: number;
  dedup_eligible: number;
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
