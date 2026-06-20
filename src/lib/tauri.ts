import { invoke } from "@tauri-apps/api/core";
import type {
  ApiTestResult,
  AppSettings,
  Category,
  AutomationStatus,
  BackupExportResult,
  DashboardStats,
  FetchResult,
  Post,
  PresetSource,
  PublishLog,
  PublishResult,
  RegenerateQueueImagesResult,
  DuplicatesOverview,
  UnpublishResult,
  RssPreviewItem,
  LocalModelsOverview,
  Source,
  VkOAuthResult,
} from "./types";

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke("save_settings", { settings });
}

export async function testVk(): Promise<ApiTestResult> {
  return invoke("test_vk");
}

export async function vkOauthAuthorize(): Promise<VkOAuthResult> {
  return invoke("vk_oauth_authorize");
}

export async function testTelegram(): Promise<ApiTestResult> {
  return invoke("test_telegram");
}

export async function testDeepseek(): Promise<ApiTestResult> {
  return invoke("test_deepseek");
}

export async function getLocalLlmStatus(): Promise<LocalModelsOverview> {
  return invoke("get_local_models_overview");
}

export async function getLocalModelsOverview(): Promise<LocalModelsOverview> {
  return invoke("get_local_models_overview");
}

export async function downloadLocalServer(): Promise<void> {
  return invoke("download_local_server");
}

export async function downloadLocalModel(modelId: string): Promise<void> {
  return invoke("download_local_model", { modelId });
}

export async function pauseLocalModelDownload(modelId: string): Promise<void> {
  return invoke("pause_local_model_download", { modelId });
}

export async function cancelLocalModelDownload(modelId: string): Promise<void> {
  return invoke("cancel_local_model_download", { modelId });
}

export async function cancelLocalServerDownload(): Promise<void> {
  return invoke("cancel_local_server_download");
}

export async function deleteLocalModelPartial(modelId: string): Promise<void> {
  return invoke("delete_local_model_partial", { modelId });
}

export async function deleteLocalModel(modelId: string): Promise<void> {
  return invoke("delete_local_model", { modelId });
}

export async function addCustomLocalModel(
  name: string,
  description: string,
  downloadUrl: string
): Promise<void> {
  return invoke("add_custom_local_model", { name, description, downloadUrl });
}

export async function removeCustomLocalModel(modelId: string): Promise<void> {
  return invoke("remove_custom_local_model", { modelId });
}

export async function setLocalModel(modelId: string): Promise<void> {
  return invoke("set_local_model", { modelId });
}

export async function setLocalDedupModel(modelId: string): Promise<void> {
  return invoke("set_local_dedup_model", { modelId });
}

export async function startLocalLlmDownload(): Promise<void> {
  return invoke("start_local_llm_download");
}

export async function testProxy(): Promise<ApiTestResult> {
  return invoke("test_proxy");
}

export async function pickProxyFile(): Promise<string> {
  return invoke("pick_proxy_file");
}

export async function pickWatermarkFile(): Promise<string> {
  return invoke("pick_watermark_file");
}

export async function getWatermarkNaturalSize(
  localRef: string
): Promise<{ width: number; height: number }> {
  return invoke("get_watermark_natural_size", { localRef });
}

export async function fetchProxyList(url: string): Promise<string> {
  return invoke("fetch_proxy_list", { url });
}

export async function resolveLocalImagePath(localRef: string): Promise<string> {
  return invoke("resolve_local_image_path", { localRef });
}

export async function readLocalImageDataUrl(localRef: string): Promise<string> {
  return invoke("read_local_image_data_url", { localRef });
}

export async function getCategories(): Promise<Category[]> {
  return invoke("get_categories");
}

export async function updateCategory(category: Category): Promise<void> {
  return invoke("update_category", { category });
}

export async function getSources(): Promise<Source[]> {
  return invoke("get_sources");
}

export async function addSource(
  url: string,
  name: string,
  categoryId: number | null
): Promise<Source> {
  return invoke("add_source", { url, name, categoryId });
}

export async function updateSource(source: Source): Promise<void> {
  return invoke("update_source", { source });
}

export async function deleteSource(id: number): Promise<void> {
  return invoke("delete_source", { id });
}

export async function getPresetSources(): Promise<PresetSource[]> {
  return invoke("get_preset_sources");
}

export async function addPresetSources(urls: string[]): Promise<number> {
  return invoke("add_preset_sources", { urls });
}

export async function previewSource(url: string): Promise<RssPreviewItem[]> {
  return invoke("preview_source", { url });
}

export async function getPosts(status?: string): Promise<Post[]> {
  return invoke("get_posts", { status: status ?? null });
}

export async function getPost(id: number): Promise<Post> {
  return invoke("get_post", { id });
}

export async function updatePost(post: Post): Promise<void> {
  return invoke("update_post", { post });
}

export async function refreshPostSource(id: number): Promise<Post> {
  return invoke("refresh_post_source", { id });
}

export async function deletePost(id: number): Promise<void> {
  return invoke("delete_post", { id });
}

export async function reprocessPost(id: number): Promise<Post> {
  return invoke("reprocess_post", { id });
}

export async function fetchNews(): Promise<FetchResult> {
  return invoke("fetch_news");
}

export async function cancelFetchNews(): Promise<boolean> {
  return invoke("cancel_fetch_news");
}

export async function getAutomationStatus(): Promise<AutomationStatus> {
  return invoke("get_automation_status");
}

export async function processPostWithAi(id: number): Promise<Post> {
  return invoke("process_post_with_ai", { id });
}

export async function publishPost(id: number): Promise<PublishResult> {
  return invoke("publish_post", { id });
}

export async function unpublishPost(id: number): Promise<UnpublishResult> {
  return invoke("unpublish_post", { id });
}

export async function deleteQueuePosts(): Promise<number> {
  return invoke("delete_queue_posts");
}

export async function regenerateQueueImages(): Promise<RegenerateQueueImagesResult> {
  return invoke("regenerate_queue_images");
}

export async function resetAllData(): Promise<void> {
  return invoke("reset_all_data");
}

export async function pickBackupDirectory(): Promise<string> {
  return invoke("pick_backup_directory");
}

export async function exportBackupManual(): Promise<BackupExportResult> {
  return invoke("export_backup_manual");
}

export async function importBackup(): Promise<void> {
  return invoke("import_backup");
}

export async function getDashboardStats(): Promise<DashboardStats> {
  return invoke("get_dashboard_stats");
}

export async function getPublishHistory(): Promise<PublishLog[]> {
  return invoke("get_publish_history");
}

export async function getPublishedPosts(): Promise<Post[]> {
  return invoke("get_published_posts");
}

export async function getRecentPublishedPosts(limit = 5): Promise<Post[]> {
  return invoke("get_recent_published_posts", { limit });
}

export async function getDuplicatesOverview(): Promise<DuplicatesOverview> {
  return invoke("get_duplicates_overview");
}
