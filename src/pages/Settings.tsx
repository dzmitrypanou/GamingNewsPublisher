import { useEffect, useMemo, useRef, useState } from "react";
import { FileUp, Link2, Loader2, Pause, Save, TestTube, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  getSettings,
  saveSettings,
  resetAllData,
  testVk,
  testTelegram,
  testDeepseek,
  testProxy,
  pickProxyFile,
  fetchProxyList,
  getLocalModelsOverview,
  downloadLocalServer,
  downloadLocalModel,
  cancelLocalModelDownload,
  pauseLocalModelDownload,
  cancelLocalServerDownload,
  deleteLocalModel,
  deleteLocalModelPartial,
  addCustomLocalModel,
  removeCustomLocalModel,
  setLocalModel,
} from "@/lib/tauri";
import { listen } from "@tauri-apps/api/event";
import { dialog } from "@/lib/dialog";
import type { AppSettings, ApiTestResult, LocalModelInfo, LocalModelsOverview } from "@/lib/types";
import { cn, countProxyLines, mergeProxyLists } from "@/lib/utils";
import { WatermarkEditor } from "@/components/settings/WatermarkEditor";

const DEFAULT_PROMPT = `Переведи игровую новость на {language} и перепиши для соцсетей VK и Telegram.
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
{web_context}`;

const defaultSettings: AppSettings = {
  vk_token: "",
  vk_group_id: "",
  telegram_bot_token: "",
  telegram_channel_id: "",
  deepseek_api_key: "",
  deepseek_model: "deepseek-chat",
  ai_provider: "local",
  ai_generation_provider: "local",
  ai_duplicate_provider: "local",
  local_model_id: "vikhr-nemo-12b-instruct",
  local_dedup_model_id: "vikhr-nemo-12b-instruct",
  local_llm_device: "gpu",
  local_llm_gpu_layers: 28,
  ai_prompt_template: DEFAULT_PROMPT,
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
  post_language: "ru",
  proxy_enabled: false,
  proxy_type: "http",
  proxy_list: "",
  post_image_width: 1280,
  post_image_height: 720,
  watermark_enabled: false,
  watermark_image: "",
  watermark_opacity: 85,
  watermark_scale_percent: 18,
  watermark_position_mode: "preset",
  watermark_preset: "bottom_right",
  watermark_margin_x: 24,
  watermark_margin_y: 24,
  watermark_x: 0,
  watermark_y: 0,
  watermark_size_mode: "scale",
  watermark_width_px: 0,
  watermark_height_px: 0,
  web_context_enabled: true,
  web_search_provider: "article_only",
  tavily_api_key: "",
  ai_duplicate_window_days: 30,
  ai_duplicate_check_limit: 200,
  ai_duplicate_llm_top_k: 50,
};

const PANEL_HEADER = "p-4 pb-2";
const PANEL_CONTENT = "space-y-3 p-4 pt-0";
const PANEL_BOX = "space-y-2 rounded-lg border border-border bg-secondary/20 p-3";
const PANEL_INSET = "space-y-2 rounded-lg border border-border bg-secondary/10 p-3";

export function Settings() {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testResults, setTestResults] = useState<Record<string, ApiTestResult>>({});
  const [testing, setTesting] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const savedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [resetting, setResetting] = useState(false);
  const [proxyUrl, setProxyUrl] = useState("");
  const [proxyImporting, setProxyImporting] = useState<"file" | "url" | null>(null);
  const [localLlm, setLocalLlm] = useState<LocalModelsOverview | null>(null);
  const [customModelName, setCustomModelName] = useState("");
  const [customModelDescription, setCustomModelDescription] = useState("");
  const [customModelUrl, setCustomModelUrl] = useState("");
  const [addingCustomModel, setAddingCustomModel] = useState(false);
  const [showCustomModelForm, setShowCustomModelForm] = useState(false);

  const formatGb = (bytes: number) => {
    if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} ГБ`;
    if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} МБ`;
    if (bytes > 0) return `${Math.max(1, Math.round(bytes / 1024))} КБ`;
    return "0";
  };

  const proxyCount = useMemo(() => countProxyLines(settings.proxy_list), [settings.proxy_list]);
  const localNeeded =
    settings.ai_generation_provider === "local" ||
    settings.ai_duplicate_provider === "local";
  const cloudNeeded =
    settings.ai_generation_provider === "cloud" ||
    settings.ai_duplicate_provider === "cloud";
  const generationConfigured =
    settings.ai_generation_provider === "local"
      ? !!localLlm?.ready
      : settings.ai_generation_provider === "cloud"
        ? !!settings.deepseek_api_key
        : false;
  const duplicateConfigured =
    settings.ai_duplicate_provider === "local"
      ? !!localLlm?.dedup_ready
      : settings.ai_duplicate_provider === "cloud"
        ? !!settings.deepseek_api_key
        : false;
  const duplicateCheckAvailable =
    settings.ai_duplicate_provider !== "off" && duplicateConfigured;

  useEffect(() => {
    return () => {
      if (savedTimerRef.current) {
        clearTimeout(savedTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    getSettings()
      .then((s) =>
        setSettings({
          ...defaultSettings,
          ...s,
          ai_generation_provider:
            s.ai_generation_provider ?? s.ai_provider ?? "cloud",
          ai_duplicate_provider:
            s.ai_duplicate_provider ?? s.ai_provider ?? "cloud",
        })
      )
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    const refresh = () => getLocalModelsOverview().then(setLocalLlm).catch(console.error);
    refresh();
    const timer = setInterval(refresh, 2000);
    const unlisten = listen<LocalModelsOverview>("local-llm-download-progress", (e) => {
      setLocalLlm(e.payload);
    });
    return () => {
      clearInterval(timer);
      unlisten.then((fn) => fn());
    };
  }, []);

  const activeModel = localLlm?.models.find((m) => m.is_active);
  const llmModels = useMemo(
    () =>
      (localLlm?.models ?? []).filter(
        (m) => m.model_kind === "llm" && !m.deprecated_reason
      ),
    [localLlm?.models]
  );

  type RuntimeStatus = { label: string; ok: boolean; detail?: string; error?: string | null };

  const generationRuntimeStatus = (): RuntimeStatus => {
    if (settings.ai_generation_provider !== "local") {
      return { label: "Не используется", ok: true, detail: "выбран API или выкл" };
    }
    if (!localLlm?.server_installed) {
      return { label: "llama-server не установлен", ok: false };
    }
    const active = llmModels.find((m) => m.is_active);
    if (!active?.installed) {
      return {
        label: "LLM не установлена",
        ok: false,
        detail: activeModel?.name ?? settings.local_model_id,
      };
    }
    if (localLlm.ready) {
      return {
        label: "Готово",
        ok: true,
        detail: `${active.name} · -ngl ${localLlm.active_ngl}`,
      };
    }
    return {
      label: "Сервер или LLM не готовы",
      ok: false,
      error: localLlm.runtime_error,
    };
  };

  const dedupRuntimeStatus = (): RuntimeStatus => {
    if (settings.ai_duplicate_provider !== "local") {
      return { label: "Не используется", ok: true, detail: "выбран API или выкл" };
    }
    if (!localLlm?.server_installed) {
      return { label: "llama-server не установлен", ok: false };
    }
    const active = llmModels.find((m) => m.is_active);
    if (!active?.installed) {
      return {
        label: "LLM не установлена",
        ok: false,
        detail: activeModel?.name ?? settings.local_model_id,
      };
    }
    if (localLlm.dedup_ready) {
      return {
        label: "Готово",
        ok: true,
        detail: `${active.name} · LLM-сравнение`,
      };
    }
    return {
      label: "LLM не запущена",
      ok: false,
      error: localLlm.dedup_runtime_error,
    };
  };

  const renderRuntimeStatus = (status: RuntimeStatus) => (
    <div className="space-y-0.5">
      <p className={cn("text-sm font-medium", status.ok ? "text-success" : "text-warning")}>
        {status.label}
      </p>
      {status.detail && <p className="text-xs text-muted-foreground">{status.detail}</p>}
      {status.error && <p className="text-xs text-destructive">{status.error}</p>}
    </div>
  );

  const renderRuntimePanel = (title: string, status: RuntimeStatus) => (
    <div className="rounded-lg border border-border bg-secondary/20 p-3">
      <p className="mb-1.5 text-xs font-medium text-muted-foreground">{title}</p>
      {renderRuntimeStatus(status)}
    </div>
  );

  const modelKindLabel = (kind: LocalModelInfo["model_kind"]) => {
    if (kind === "encoder") return "Энкодер";
    if (kind === "nli") return "NLI";
    return "LLM";
  };

  const handleSelectModel = async (modelId: string) => {
    try {
      await setLocalModel(modelId);
      update("local_model_id", modelId);
      update("local_dedup_model_id", modelId);
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    try {
      await downloadLocalModel(modelId);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка загрузки", variant: "error" });
    }
  };

  const handleDeleteModel = async (
    modelId: string,
    name: string,
    isActive: boolean,
    fallbackModelName?: string
  ) => {
    const message = isActive
      ? fallbackModelName
        ? `Активная модель «${name}» будет удалена с диска. Активной станет «${fallbackModelName}».`
        : `Удалить активную модель «${name}» с диска?`
      : `Удалить модель «${name}» с диска?`;

    if (
      !(await dialog.confirm(message, {
        title: "Удалить модель",
        confirmText: "Удалить",
        destructive: true,
      }))
    ) {
      return;
    }
    try {
      await deleteLocalModel(modelId);
      const overview = await getLocalModelsOverview();
      setLocalLlm(overview);
      setSettings((prev) => ({
        ...prev,
        local_model_id: overview.active_model_id,
        local_dedup_model_id: overview.active_dedup_model_id,
      }));
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleDeletePartial = async (modelId: string, name: string) => {
    if (
      !(await dialog.confirm(`Удалить недоскачанный файл «${name}»?`, {
        title: "Удалить загрузку",
        confirmText: "Удалить",
        destructive: true,
      }))
    ) {
      return;
    }
    try {
      await deleteLocalModelPartial(modelId);
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleAddCustomModel = async () => {
    const name = customModelName.trim();
    const url = customModelUrl.trim();
    if (!name || !url) {
      await dialog.alert("Укажите название и URL файла .gguf", {
        title: "Добавление модели",
        variant: "error",
      });
      return;
    }
    setAddingCustomModel(true);
    try {
      await addCustomLocalModel(name, customModelDescription.trim(), url);
      setCustomModelName("");
      setCustomModelDescription("");
      setCustomModelUrl("");
      setShowCustomModelForm(false);
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setAddingCustomModel(false);
    }
  };

  const handleRemoveCustomModel = async (modelId: string, name: string) => {
    if (
      !(await dialog.confirm(`Удалить пользовательскую модель «${name}» из списка?`, {
        title: "Удалить модель",
        confirmText: "Удалить",
        destructive: true,
      }))
    ) {
      return;
    }
    try {
      await removeCustomLocalModel(modelId);
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const formatModelSize = (model: { size_hint_bytes: number; file_bytes: number; installed: boolean }) => {
    if (model.installed) {
      return formatGb(model.file_bytes);
    }
    if (model.size_hint_bytes > 0) {
      return `~${formatGb(model.size_hint_bytes)}`;
    }
    return "размер неизвестен";
  };

  const renderModelCard = (model: LocalModelInfo) => {
    const peers = llmModels;
    const isActive = model.is_active;
    const otherInstalled = peers.filter((m) => m.installed && m.id !== model.id);
    const canDelete =
      model.installed &&
      !model.downloading &&
      (!isActive || otherInstalled.length > 0);
    const canRemoveCustom =
      model.is_custom &&
      !model.installed &&
      !model.downloading &&
      !model.has_partial_download &&
      !model.install_invalid;
    const fallbackModel = otherInstalled[0];
    const canSelect =
      model.installed &&
      !isActive &&
      !model.downloading &&
      !model.deprecated_reason &&
      model.model_kind === "llm";

    return (
      <div
        key={model.id}
        className={cn(
          "rounded-lg border p-3",
          isActive ? "border-primary/50 bg-primary/5" : "border-border"
        )}
      >
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">{model.name}</p>
            <p className="mt-0.5 text-xs text-muted-foreground">{model.description}</p>
          </div>
          <div className="flex shrink-0 flex-col items-end gap-1">
            {isActive && (
              <span className="text-xs text-primary">Активна</span>
            )}
            {model.recommended && (
              <span className="text-xs text-success">Рекомендуется</span>
            )}
            {model.is_custom && (
              <span className="text-xs text-muted-foreground">Своя</span>
            )}
            <span className="text-xs text-muted-foreground">
              {modelKindLabel(model.model_kind)}
            </span>
            {model.deprecated_reason && (
              <span className="max-w-[140px] text-right text-xs text-warning">
                {model.deprecated_reason}
              </span>
            )}
          </div>
        </div>

        {isActive && model.installed && !model.downloading && (
          <div className="mt-2 rounded-md border border-border bg-background/40 px-2 py-1.5">
            {renderRuntimeStatus(generationRuntimeStatus())}
          </div>
        )}

        {model.downloading && (
          <div className="mt-3 space-y-1.5">
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs text-muted-foreground">
                Загрузка: {model.progress_pct.toFixed(0)}%
                {model.file_bytes > 0 ? ` · ${formatGb(model.file_bytes)}` : ""}
              </span>
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-7 px-2 text-xs"
                  onClick={() => void handlePauseModelDownload(model.id)}
                >
                  <Pause className="h-3 w-3" />
                  Пауза
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                  onClick={() => void handleCancelModelDownload(model.id)}
                >
                  <X className="h-3 w-3" />
                  Отмена
                </Button>
              </div>
            </div>
            <DownloadProgressBar value={model.progress_pct} />
          </div>
        )}

        {!model.downloading && model.has_partial_download && !model.installed && (
          <div className="mt-3 space-y-1.5">
            <span className="text-xs text-muted-foreground">
              Пауза: {model.progress_pct.toFixed(0)}%
              {model.file_bytes > 0 ? ` · ${formatGb(model.file_bytes)}` : ""}
            </span>
            <DownloadProgressBar value={model.progress_pct} />
          </div>
        )}

        {model.install_invalid && !model.downloading && (
          <p className="mt-2 text-xs text-destructive">
            Файл повреждён или неполный — удалите и скачайте заново
          </p>
        )}

        {model.download_error && !model.downloading && (
          <p className="mt-2 text-xs text-destructive">{model.download_error}</p>
        )}

        <div className="mt-2 flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted-foreground">
            {model.installed
              ? formatGb(model.file_bytes)
              : (model.downloading || model.has_partial_download) && model.file_bytes > 0
                ? model.size_hint_bytes > 0
                  ? `${formatGb(model.file_bytes)} / ~${formatGb(model.size_hint_bytes)}`
                  : `${formatGb(model.file_bytes)} / ${formatModelSize(model)}`
                : formatModelSize(model)}
          </span>
          {!model.installed && !model.downloading && (
            <>
              <Button
                size="sm"
                variant="outline"
                onClick={() => handleDownloadModel(model.id)}
              >
                {model.has_partial_download || model.install_invalid
                  ? "Продолжить скачивание"
                  : "Скачать"}
              </Button>
              {(model.has_partial_download || model.install_invalid) && (
                <Button
                  size="sm"
                  variant="outline"
                  className="text-destructive hover:text-destructive"
                  onClick={() => void handleDeletePartial(model.id, model.name)}
                >
                  <Trash2 className="h-3 w-3" />
                  Удалить
                </Button>
              )}
            </>
          )}
          {canSelect && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => void handleSelectModel(model.id)}
            >
              Выбрать
            </Button>
          )}
          {canRemoveCustom && (
            <Button
              size="sm"
              variant="outline"
              className="text-destructive hover:text-destructive"
              onClick={() => void handleRemoveCustomModel(model.id, model.name)}
            >
              <Trash2 className="h-3 w-3" />
              Удалить
            </Button>
          )}
          {canDelete && (
            <Button
              size="sm"
              variant="outline"
              className="text-destructive hover:text-destructive"
              onClick={() =>
                handleDeleteModel(
                  model.id,
                  model.name,
                  isActive,
                  fallbackModel?.name
                )
              }
            >
              <Trash2 className="h-3 w-3" />
              Удалить
            </Button>
          )}
          {model.installed && isActive && !model.downloading && otherInstalled.length === 0 && (
            <span className="text-xs text-muted-foreground">
              Чтобы удалить — сначала скачайте другую модель
            </span>
          )}
        </div>
      </div>
    );
  };

  const handlePauseModelDownload = async (modelId: string) => {
    try {
      await pauseLocalModelDownload(modelId);
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleCancelModelDownload = async (modelId: string) => {
    try {
      await cancelLocalModelDownload(modelId);
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleCancelServerDownload = async () => {
    try {
      await cancelLocalServerDownload();
      await getLocalModelsOverview().then(setLocalLlm);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleInstallServer = async () => {
    try {
      await downloadLocalServer();
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const update = (key: keyof AppSettings, value: string | number | boolean) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
    setSaved(false);
  };

  const patchSettings = (patch: Partial<AppSettings>) => {
    setSettings((prev) => ({ ...prev, ...patch }));
    setSaved(false);
  };

  const handleResetAll = async () => {
    if (
      !(await dialog.confirm(
        "Будут удалены все посты, история публикаций и журнал парсинга. Повторный сбор новостей станет возможен с нуля.",
        {
          title: "СБРОСИТЬ ВСЕ?",
          confirmText: "Сбросить",
          destructive: true,
          variant: "error",
        }
      ))
    ) {
      return;
    }
    if (
      !(await dialog.confirm("Это действие необратимо. Продолжить?", {
        title: "Подтверждение",
        confirmText: "Да, удалить всё",
        destructive: true,
        variant: "error",
      }))
    ) {
      return;
    }
    setResetting(true);
    try {
      await resetAllData();
      await dialog.alert("Все данные очищены. Можно снова собирать новости.", {
        title: "Готово",
        variant: "success",
      });
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setResetting(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    if (savedTimerRef.current) {
      clearTimeout(savedTimerRef.current);
      savedTimerRef.current = null;
    }
    try {
      await saveSettings(settings);
      setSaved(true);
      savedTimerRef.current = setTimeout(() => {
        setSaved(false);
        savedTimerRef.current = null;
      }, 1500);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setSaving(false);
    }
  };

  const handleImportProxyFile = async () => {
    setProxyImporting("file");
    try {
      const content = await pickProxyFile();
      update("proxy_list", mergeProxyLists(settings.proxy_list, content));
    } catch (e) {
      const message = String(e);
      if (!message.includes("не выбран")) {
        await dialog.alert(message, { title: "Ошибка импорта", variant: "error" });
      }
    } finally {
      setProxyImporting(null);
    }
  };

  const handleImportProxyUrl = async () => {
    if (!proxyUrl.trim()) {
      await dialog.alert("Вставьте ссылку на файл со списком прокси", {
        title: "Нет ссылки",
        variant: "info",
      });
      return;
    }
    setProxyImporting("url");
    try {
      const content = await fetchProxyList(proxyUrl.trim());
      update("proxy_list", mergeProxyLists(settings.proxy_list, content));
      setProxyUrl("");
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка загрузки", variant: "error" });
    } finally {
      setProxyImporting(null);
    }
  };

  const handleClearProxyList = async () => {
    if (!settings.proxy_list.trim()) return;
    if (
      !(await dialog.confirm("Очистить весь список прокси?", {
        title: "Очистить список",
        confirmText: "Очистить",
        destructive: true,
      }))
    ) {
      return;
    }
    update("proxy_list", "");
  };

  const handleTest = async (platform: "vk" | "telegram" | "deepseek" | "proxy") => {
    setTesting(platform);
    try {
      await saveSettings(settings);
      const fn =
        platform === "vk"
          ? testVk
          : platform === "telegram"
            ? testTelegram
            : platform === "proxy"
              ? testProxy
              : testDeepseek;
      const result = await fn();
      setTestResults((prev) => ({ ...prev, [platform]: result }));
    } catch (e) {
      setTestResults((prev) => ({
        ...prev,
        [platform]: { success: false, message: String(e) },
      }));
    } finally {
      setTesting(null);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Настройки</h1>
          <p className="text-sm text-muted-foreground">API-ключи и параметры приложения</p>
        </div>
        <Button onClick={handleSave} disabled={saving || saved}>
          {saving ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Save className="h-4 w-4" />
          )}
          {saved ? "Сохранено" : "Сохранить"}
        </Button>
      </div>

      <div className="flex flex-col gap-5 xl:flex-row xl:items-start">
        <div className="flex min-w-0 flex-1 flex-col gap-5">
        <Card>
          <CardHeader className={PANEL_HEADER}>
            <CardTitle className="text-base">Публикация</CardTitle>
            <CardDescription className="text-xs">VK и Telegram</CardDescription>
          </CardHeader>
          <CardContent className={PANEL_CONTENT}>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-3">
                <p className="text-sm font-medium text-[#0077FF]">VKontakte</p>
                <div className="space-y-2">
                  <Label className="text-xs">Access Token</Label>
                  <Input
                    type="password"
                    value={settings.vk_token}
                    onChange={(e) => update("vk_token", e.target.value)}
                    placeholder="vk1.a...."
                  />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">ID группы</Label>
                  <Input
                    value={settings.vk_group_id}
                    onChange={(e) => update("vk_group_id", e.target.value)}
                    placeholder="123456789"
                  />
                </div>
                <TestButton
                  platform="vk"
                  testing={testing}
                  result={testResults.vk}
                  onTest={handleTest}
                />
              </div>
              <div className="space-y-3 sm:border-l sm:border-border sm:pl-4">
                <p className="text-sm font-medium text-[#2AABEE]">Telegram</p>
                <div className="space-y-2">
                  <Label className="text-xs">Bot Token</Label>
                  <Input
                    type="password"
                    value={settings.telegram_bot_token}
                    onChange={(e) => update("telegram_bot_token", e.target.value)}
                    placeholder="123456:ABC..."
                  />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">Channel ID</Label>
                  <Input
                    value={settings.telegram_channel_id}
                    onChange={(e) => update("telegram_channel_id", e.target.value)}
                    placeholder="@mychannel или -1001234567890"
                  />
                </div>
                <TestButton
                  platform="telegram"
                  testing={testing}
                  result={testResults.telegram}
                  onTest={handleTest}
                />
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className={PANEL_HEADER}>
            <CardTitle className="text-base">Использование AI</CardTitle>
            <CardDescription className="text-xs">
              Генерация постов и проверка дублей — локально или через API
            </CardDescription>
          </CardHeader>
          <CardContent className={PANEL_CONTENT}>
            <div className={cn(PANEL_BOX, "grid gap-3 sm:grid-cols-2")}>
              <div className="space-y-1.5">
                <Label className="text-xs">Генерация постов</Label>
                <div className="grid grid-cols-3 gap-1 rounded-md border border-border bg-background/50 p-0.5">
                  {(
                    [
                      ["local", "Локально"],
                      ["cloud", "API"],
                      ["off", "Выкл"],
                    ] as const
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => update("ai_generation_provider", value)}
                      className={cn(
                        "rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                        settings.ai_generation_provider === value
                          ? "bg-primary text-primary-foreground"
                          : "text-muted-foreground hover:text-foreground"
                      )}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">Проверка дублей</Label>
                <div className="grid grid-cols-3 gap-1 rounded-md border border-border bg-background/50 p-0.5">
                  {(
                    [
                      ["local", "Локально"],
                      ["cloud", "API"],
                      ["off", "Выкл"],
                    ] as const
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => update("ai_duplicate_provider", value)}
                      className={cn(
                        "rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                        settings.ai_duplicate_provider === value
                          ? "bg-primary text-primary-foreground"
                          : "text-muted-foreground hover:text-foreground"
                      )}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            {localNeeded && (
              <div className="grid gap-3 sm:grid-cols-2">
                {settings.ai_generation_provider === "local" &&
                  renderRuntimePanel("Генерация (LLM)", generationRuntimeStatus())}
                {settings.ai_duplicate_provider === "local" &&
                  renderRuntimePanel("Дубли (LLM)", dedupRuntimeStatus())}
              </div>
            )}

            {cloudNeeded ? (
              <div className={PANEL_BOX}>
                <Label className="text-xs">Облачный API (DeepSeek)</Label>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">API Key</Label>
                  <Input
                    type="password"
                    value={settings.deepseek_api_key}
                    onChange={(e) => update("deepseek_api_key", e.target.value)}
                    placeholder="sk-..."
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-muted-foreground">Модель</Label>
                  <Input
                    value={settings.deepseek_model}
                    onChange={(e) => update("deepseek_model", e.target.value)}
                    placeholder="deepseek-chat"
                  />
                </div>
              </div>
            ) : null}

            <div className="space-y-1.5">
              <Label className="text-xs">Шаблон промпта</Label>
              <Textarea
                value={settings.ai_prompt_template}
                onChange={(e) => update("ai_prompt_template", e.target.value)}
                rows={5}
                className="font-mono text-xs"
              />
              <p className="text-xs text-muted-foreground">
                Переменные: {"{title}"}, {"{description}"}, {"{category}"}, {"{language}"},{" "}
                {"{web_context}"}
              </p>
            </div>

            <div className={PANEL_BOX}>
              <div className="flex items-center justify-between gap-3">
                <div>
                  <Label className="text-sm">Контекст из интернета</Label>
                  <p className="text-xs text-muted-foreground">
                    Перед генерацией подтягивается текст статьи по ссылке поста
                  </p>
                </div>
                <Switch
                  checked={settings.web_context_enabled}
                  onCheckedChange={(v) => update("web_context_enabled", v)}
                />
              </div>
              {settings.web_context_enabled && (
                <div className="space-y-2">
                  <Label className="text-xs">Источник контекста</Label>
                  <div className="grid grid-cols-2 gap-1 rounded-md border border-border bg-background/50 p-0.5">
                    {(
                      [
                        ["article_only", "Статья по URL"],
                        ["tavily", "Статья + Tavily"],
                      ] as const
                    ).map(([value, label]) => (
                      <button
                        key={value}
                        type="button"
                        onClick={() => update("web_search_provider", value)}
                        className={cn(
                          "rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                          settings.web_search_provider === value
                            ? "bg-primary text-primary-foreground"
                            : "text-muted-foreground hover:text-foreground"
                        )}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                  {settings.web_search_provider === "tavily" && (
                    <div className="space-y-1.5">
                      <Label className="text-xs text-muted-foreground">Tavily API Key</Label>
                      <Input
                        type="password"
                        value={settings.tavily_api_key}
                        onChange={(e) => update("tavily_api_key", e.target.value)}
                        placeholder="tvly-..."
                      />
                    </div>
                  )}
                </div>
              )}
            </div>
            <div className="flex items-center justify-between rounded-lg border border-border px-3 py-2">
              <div>
                <Label className="text-sm">Проверка дублей с помощью AI</Label>
                <p className="text-xs text-muted-foreground">
                  {settings.ai_duplicate_provider === "off"
                    ? "Выберите провайдера для проверки дублей выше."
                    : settings.ai_duplicate_provider === "local"
                      ? `URL и заголовки — по всей базе; LLM сравнивает до ${settings.ai_duplicate_llm_top_k} кандидатов (до ${Math.min(settings.ai_dedup_concurrency, 2)} параллельно).`
                      : "URL и заголовки — по всей базе; LLM — до top-K кандидатов через облачный API."}
                </p>
                {settings.ai_duplicate_provider === "cloud" && !settings.deepseek_api_key && (
                  <p className="mt-1 text-xs text-warning">Укажите API ключ DeepSeek</p>
                )}
                {settings.ai_generation_provider === "local" && !localLlm?.ready && (
                  <p className="mt-1 text-xs text-warning">
                    {generationRuntimeStatus().label}
                    {localLlm?.runtime_error ? `: ${localLlm.runtime_error}` : ""}
                  </p>
                )}
                {settings.ai_duplicate_provider === "local" && !localLlm?.dedup_ready && (
                  <p className="mt-1 text-xs text-warning">
                    {dedupRuntimeStatus().label}
                    {localLlm?.dedup_runtime_error ? `: ${localLlm.dedup_runtime_error}` : ""}
                  </p>
                )}
              </div>
              <Switch
                checked={settings.ai_duplicate_check}
                disabled={!duplicateCheckAvailable}
                onCheckedChange={(v) => update("ai_duplicate_check", v)}
              />
            </div>
            {settings.ai_duplicate_check && settings.ai_duplicate_provider !== "off" && (
              <div className={PANEL_BOX}>
                <p className="text-xs text-muted-foreground">
                  Сначала проверяются URL и точное совпадение заголовка по всей базе. Затем из
                  постов за указанный период отбираются кандидаты; LLM сравнивает top-K
                  (похожие по эвристике — всегда). Для Cloud рекомендуется top-K 50–100, окно 0 или
                  60+ дней.
                </p>
                <div className="grid grid-cols-3 gap-2">
                  <div className="space-y-1">
                    <Label className="text-xs">Окно (дней)</Label>
                    <Input
                      type="number"
                      min={0}
                      max={365}
                      value={settings.ai_duplicate_window_days}
                      onChange={(e) =>
                        update("ai_duplicate_window_days", parseInt(e.target.value) || 0)
                      }
                    />
                    <p className="text-[10px] text-muted-foreground">0 = без ограничения</p>
                  </div>
                  <div className="space-y-1">
                    <Label className="text-xs">Кандидатов из БД</Label>
                    <Input
                      type="number"
                      min={10}
                      max={1000}
                      value={settings.ai_duplicate_check_limit}
                      onChange={(e) =>
                        update("ai_duplicate_check_limit", parseInt(e.target.value) || 200)
                      }
                    />
                  </div>
                  <div className="space-y-1">
                    <Label className="text-xs">LLM top-K</Label>
                    <Input
                      type="number"
                      min={1}
                      max={100}
                      value={settings.ai_duplicate_llm_top_k}
                      onChange={(e) =>
                        update("ai_duplicate_llm_top_k", parseInt(e.target.value) || 50)
                      }
                    />
                  </div>
                </div>
              </div>
            )}
            <TestButton
              platform="deepseek"
              testing={testing}
              result={testResults.deepseek}
              onTest={handleTest}
              disabled={!generationConfigured}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader className={PANEL_HEADER}>
            <CardTitle className="text-base">Общие</CardTitle>
            <CardDescription className="text-xs">Автоматизация и интервалы</CardDescription>
          </CardHeader>
          <CardContent className={PANEL_CONTENT}>
            <div className="flex items-center justify-between">
              <div>
                <Label>Автопарсинг</Label>
                <p className="text-xs text-muted-foreground">
                  Автоматически собирать новости из RSS по расписанию
                </p>
              </div>
              <Switch
                checked={settings.auto_fetch}
                onCheckedChange={(v) => update("auto_fetch", v)}
              />
            </div>
            {settings.auto_fetch && (
              <div className="space-y-2">
                <Label>Интервал автопарсинга (мин)</Label>
                <Input
                  type="number"
                  min={5}
                  max={1440}
                  value={settings.fetch_interval_minutes}
                  onChange={(e) =>
                    update("fetch_interval_minutes", parseInt(e.target.value) || 30)
                  }
                />
              </div>
            )}
            <div className="space-y-2">
              <Label>Новостей с каждого источника</Label>
              <Input
                type="number"
                min={1}
                max={50}
                value={settings.fetch_items_per_source}
                onChange={(e) =>
                  update("fetch_items_per_source", parseInt(e.target.value) || 10)
                }
              />
              <p className="text-xs text-muted-foreground">
                За сбор проверяется до (источники × это число) записей RSS. Новых постов может
                быть меньше: в лентах часто меньше записей, плюс пропуск уже собранных ссылок и
                дублей между источниками.
              </p>
            </div>
            <div className={PANEL_BOX}>
              <Label className="text-xs">Параллельность (потоки)</Label>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <Label className="text-xs">RSS-источники</Label>
                  <Input
                    type="number"
                    min={1}
                    max={20}
                    value={settings.fetch_sources_concurrency}
                    onChange={(e) =>
                      update("fetch_sources_concurrency", parseInt(e.target.value) || 6)
                    }
                  />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">Новости (картинки)</Label>
                  <Input
                    type="number"
                    min={1}
                    max={16}
                    value={settings.fetch_items_concurrency}
                    onChange={(e) =>
                      update("fetch_items_concurrency", parseInt(e.target.value) || 4)
                    }
                  />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">AI dedup</Label>
                  <Input
                    type="number"
                    min={1}
                    max={10}
                    value={settings.ai_dedup_concurrency}
                    disabled={settings.ai_duplicate_provider === "off"}
                    onChange={(e) =>
                      update("ai_dedup_concurrency", parseInt(e.target.value) || 2)
                    }
                  />
                </div>
                <div className="space-y-2">
                  <Label className="text-xs">AI обработка</Label>
                  <Input
                    type="number"
                    min={1}
                    max={10}
                    value={settings.ai_process_concurrency}
                    disabled={settings.ai_generation_provider !== "cloud"}
                    onChange={(e) =>
                      update("ai_process_concurrency", parseInt(e.target.value) || 3)
                    }
                  />
                </div>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <div>
                <Label>Автообработка AI</Label>
                <p className="text-xs text-muted-foreground">
                  Переписывать новости через выбранный провайдер генерации при сборе
                </p>
              </div>
              <Switch
                checked={settings.auto_ai_process}
                disabled={settings.ai_generation_provider === "off" || !generationConfigured}
                onCheckedChange={(v) => update("auto_ai_process", v)}
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <Label>Автоодобрение</Label>
                <p className="text-xs text-muted-foreground">
                  Автоматически одобрять посты для очереди публикации
                </p>
              </div>
              <Switch
                checked={settings.auto_approve}
                onCheckedChange={(v) => update("auto_approve", v)}
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <Label>Автопубликация</Label>
                <p className="text-xs text-muted-foreground">
                  Публиковать готовые посты в VK и Telegram по расписанию
                </p>
              </div>
              <Switch
                checked={settings.auto_publish}
                onCheckedChange={(v) => update("auto_publish", v)}
              />
            </div>
            <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
              <div className="mb-3">
                <Label className="text-destructive">Сброс данных</Label>
                <p className="mt-1 text-xs text-muted-foreground">
                  Удаляет все посты, историю публикаций и журнал парсинга. Дубли после сброса
                  снова могут быть собраны из RSS.
                </p>
              </div>
              <Button
                variant="outline"
                className="w-full border-destructive/40 text-destructive hover:bg-destructive hover:text-white"
                onClick={handleResetAll}
                disabled={resetting}
              >
                {resetting ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Trash2 className="h-4 w-4" />
                )}
                СБРОСИТЬ ВСЕ
              </Button>
            </div>

            {settings.auto_publish && (
              <>
                <div className="space-y-2">
                  <Label>Интервал автопубликации (мин)</Label>
                  <Input
                    type="number"
                    min={1}
                    max={1440}
                    value={settings.auto_publish_interval_minutes}
                    onChange={(e) =>
                      update("auto_publish_interval_minutes", Math.max(1, parseInt(e.target.value) || 1))
                    }
                  />
                  <p className="text-xs text-muted-foreground">
                    Базовый интервал между публикациями
                  </p>
                </div>
                <div className="space-y-2">
                  <Label>Разброс задержки (сек)</Label>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <Label className="text-xs text-muted-foreground">От</Label>
                      <Input
                        type="number"
                        min={0}
                        max={3600}
                        value={settings.auto_publish_jitter_seconds_min}
                        onChange={(e) =>
                          update(
                            "auto_publish_jitter_seconds_min",
                            Math.max(0, parseInt(e.target.value) || 0)
                          )
                        }
                      />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-xs text-muted-foreground">До</Label>
                      <Input
                        type="number"
                        min={0}
                        max={3600}
                        value={settings.auto_publish_jitter_seconds_max}
                        onChange={(e) =>
                          update(
                            "auto_publish_jitter_seconds_max",
                            Math.max(0, parseInt(e.target.value) || 0)
                          )
                        }
                      />
                    </div>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Случайная задержка: от {settings.auto_publish_interval_minutes} мин{" "}
                    {Math.min(
                      settings.auto_publish_jitter_seconds_min,
                      settings.auto_publish_jitter_seconds_max
                    )}{" "}
                    с до {settings.auto_publish_interval_minutes} мин{" "}
                    {Math.max(
                      settings.auto_publish_jitter_seconds_min,
                      settings.auto_publish_jitter_seconds_max
                    )}{" "}
                    с
                  </p>
                </div>
              </>
            )}
          </CardContent>
        </Card>
        </div>

        <div className="flex min-w-0 flex-1 flex-col gap-5">
        <Card>
          <CardHeader className={PANEL_HEADER}>
            <CardTitle className="text-base">Изображения постов</CardTitle>
            <CardDescription className="text-xs">
              Размер JPEG при сборе · по умолчанию 1280×720
            </CardDescription>
          </CardHeader>
          <CardContent className={PANEL_CONTENT}>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label>Ширина (px)</Label>
                <Input
                  type="number"
                  min={320}
                  max={4096}
                  value={settings.post_image_width}
                  onChange={(e) =>
                    update(
                      "post_image_width",
                      Math.min(4096, Math.max(320, parseInt(e.target.value) || 1280))
                    )
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>Высота (px)</Label>
                <Input
                  type="number"
                  min={180}
                  max={4096}
                  value={settings.post_image_height}
                  onChange={(e) =>
                    update(
                      "post_image_height",
                      Math.min(4096, Math.max(180, parseInt(e.target.value) || 720))
                    )
                  }
                />
              </div>
            </div>
            <p className="text-xs text-muted-foreground">
              Действует только для новых сборов. Уже сохранённые картинки не пересчитываются.
              Соотношение сторон:{" "}
              {(settings.post_image_width / settings.post_image_height).toFixed(2)}:1
            </p>
            <div className="border-t border-border pt-3">
              <WatermarkEditor settings={settings} onChange={update} onPatch={patchSettings} />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className={PANEL_HEADER}>
            <CardTitle className="text-base">Прокси</CardTitle>
            <CardDescription className="text-xs">
              HTTP/HTTPS/SOCKS5 · по одному на строку
            </CardDescription>
          </CardHeader>
          <CardContent className={PANEL_CONTENT}>
            <div className="flex items-center justify-between">
              <div>
                <Label>Использовать прокси</Label>
                <p className="text-xs text-muted-foreground">
                  Запросы идут через список прокси с ротацией
                </p>
              </div>
              <Switch
                checked={settings.proxy_enabled}
                onCheckedChange={(v) => update("proxy_enabled", v)}
              />
            </div>
            {settings.proxy_enabled && (
              <>
                <div className="space-y-2">
                  <Label>Тип прокси</Label>
                  <div className="grid grid-cols-3 gap-2 rounded-lg border border-border bg-secondary/20 p-1">
                    {(
                      [
                        ["http", "HTTP"],
                        ["https", "HTTPS"],
                        ["socks5", "SOCKS5"],
                      ] as const
                    ).map(([value, label]) => (
                      <button
                        key={value}
                        type="button"
                        onClick={() => update("proxy_type", value)}
                        className={cn(
                          "rounded-md px-3 py-2 text-sm font-medium transition-colors",
                          settings.proxy_type === value
                            ? "bg-primary text-primary-foreground shadow-sm"
                            : "text-muted-foreground hover:bg-accent hover:text-foreground"
                        )}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Для строк без схемы (http://, socks5://) используется выбранный тип
                  </p>
                </div>

                <div className={PANEL_INSET}>
                  <Label className="text-xs">Импорт списка</Label>
                  <div className="flex flex-col gap-2 sm:flex-row">
                    <Input
                      value={proxyUrl}
                      onChange={(e) => setProxyUrl(e.target.value)}
                      placeholder="https://example.com/proxies.txt"
                      className="font-mono text-xs"
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void handleImportProxyUrl();
                      }}
                    />
                    <Button
                      type="button"
                      variant="outline"
                      className="shrink-0"
                      disabled={proxyImporting !== null}
                      onClick={() => void handleImportProxyUrl()}
                    >
                      {proxyImporting === "url" ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Link2 className="h-4 w-4" />
                      )}
                      По ссылке
                    </Button>
                  </div>
                  <Button
                    type="button"
                    variant="outline"
                    className="w-full sm:w-auto"
                    disabled={proxyImporting !== null}
                    onClick={() => void handleImportProxyFile()}
                  >
                    {proxyImporting === "file" ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <FileUp className="h-4 w-4" />
                    )}
                    Выбрать файл .txt
                  </Button>
                </div>

                <div className="overflow-hidden rounded-lg border border-border">
                  <div className="flex items-center justify-between gap-3 border-b border-border bg-secondary/30 px-3 py-2">
                    <div className="min-w-0">
                      <p className="text-sm font-medium">Список прокси</p>
                      <p className="text-xs text-muted-foreground">
                        {proxyCount > 0
                          ? `Загружено: ${proxyCount}`
                          : "Пусто — введите вручную или импортируйте"}
                      </p>
                    </div>
                    {settings.proxy_list.trim() && (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-8 shrink-0 text-xs text-muted-foreground"
                        onClick={() => void handleClearProxyList()}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                        Очистить
                      </Button>
                    )}
                  </div>
                  <Textarea
                    value={settings.proxy_list}
                    onChange={(e) => update("proxy_list", e.target.value)}
                    rows={5}
                    className="min-h-[120px] resize-y rounded-none border-0 bg-background font-mono text-xs leading-relaxed focus-visible:ring-0 focus-visible:ring-offset-0"
                    placeholder={`192.168.1.1:8080\n10.0.0.2:3128@user:pass\nuser:pass@10.0.0.3:3128\n10.0.0.4:1080:login:password\nsocks5://1.2.3.4:1080`}
                  />
                </div>
                <p className="text-xs text-muted-foreground">
                  Форматы: IP:PORT · IP:PORT@LOGIN:PASS · LOGIN:PASS@IP:PORT ·
                  IP:PORT:LOGIN:PASS · http(s)://... · socks5://... · строки с # игнорируются
                </p>
                <TestButton
                  platform="proxy"
                  testing={testing}
                  result={testResults.proxy}
                  onTest={handleTest}
                />
              </>
            )}
          </CardContent>
        </Card>

        {localNeeded ? (
        <Card>
          <CardHeader className={PANEL_HEADER}>
            <CardTitle className="text-base">Локальные модели</CardTitle>
            <CardDescription className="text-xs">
              Vikhr-Nemo 12B или Qwen2.5 14B · генерация и дубли · app/llm/models/
            </CardDescription>
          </CardHeader>
          <CardContent className={PANEL_CONTENT}>
            <div className="grid gap-3 sm:grid-cols-2">
              {settings.ai_generation_provider === "local" &&
                renderRuntimePanel("Генерация (LLM)", generationRuntimeStatus())}
              {settings.ai_duplicate_provider === "local" &&
                renderRuntimePanel("Дубли (LLM)", dedupRuntimeStatus())}
            </div>

            {localLlm?.error && !localLlm.server_downloading && (
              <p className="text-xs text-destructive">{localLlm.error}</p>
            )}

            <div className="space-y-3">
              <div className="space-y-2">
                <Label className="text-xs">Устройство</Label>
                <div className="grid grid-cols-3 gap-1 rounded-md border border-border bg-background/50 p-0.5">
                                {(
                                  [
                                    ["cpu", "CPU"],
                                    ["gpu", "GPU"],
                                    ["hybrid", "Гибрид"],
                                  ] as const
                                ).map(([value, label]) => (
                                  <button
                                    key={value}
                                    type="button"
                                    onClick={() => update("local_llm_device", value)}
                                    className={cn(
                                      "rounded-md px-2 py-1.5 text-xs font-medium transition-colors",
                                      settings.local_llm_device === value
                                        ? "bg-primary text-primary-foreground"
                                        : "text-muted-foreground hover:text-foreground"
                                    )}
                                  >
                                    {label}
                                  </button>
                                ))}
                              </div>
                              <p className="text-xs text-muted-foreground">
                                {settings.local_llm_device === "cpu" &&
                                  "Только процессор — медленнее, без VRAM."}
                                {settings.local_llm_device === "gpu" &&
                                  "Все слои на видеокарте (Vulkan). Быстрее при достаточной VRAM."}
                                {settings.local_llm_device === "hybrid" &&
                                  "Часть слоёв на GPU, остальное в RAM — компромисс для 8 GB."}
                              </p>
                              <p className="text-xs text-muted-foreground">
                                Изменения режима и числа слоёв применяются после нажатия «Сохранить».
                              </p>
                              {settings.local_llm_device === "hybrid" && (
                                <div className="space-y-1">
                                  <Label className="text-xs">
                                    Слоёв на GPU: {settings.local_llm_gpu_layers}
                                    {activeModel ? ` (из ~${activeModel.layer_count_hint})` : ""}
                                  </Label>
                                  <Input
                                    type="range"
                                    min={1}
                                    max={99}
                                    value={settings.local_llm_gpu_layers}
                                    onChange={(e) =>
                                      update("local_llm_gpu_layers", parseInt(e.target.value) || 28)
                                    }
                                  />
                                </div>
                              )}
                            </div>

                            {!localLlm?.server_installed && (
                              <div className="space-y-2">
                                {localLlm?.server_downloading ? (
                                  <>
                                    <div className="flex items-center justify-between gap-2">
                                      <span className="text-xs text-muted-foreground">
                                        llama-server: {localLlm.server_progress_pct.toFixed(0)}%
                                      </span>
                                      <Button
                                        size="sm"
                                        variant="ghost"
                                        className="h-7 px-2 text-xs text-destructive hover:text-destructive"
                                        onClick={() => void handleCancelServerDownload()}
                                      >
                                        <X className="h-3 w-3" />
                                        Отмена
                                      </Button>
                                    </div>
                                    <DownloadProgressBar value={localLlm.server_progress_pct} />
                                  </>
                                ) : (
                                  <Button variant="outline" className="w-full" onClick={handleInstallServer}>
                                    Установить llama-server (Vulkan)
                                  </Button>
                                )}
                                {localLlm?.server_download_error && (
                                  <p className="text-xs text-destructive">{localLlm.server_download_error}</p>
                                )}
                              </div>
                            )}

                            <div className="rounded-lg border border-dashed border-border p-3">
                              {!showCustomModelForm ? (
                                <Button
                                  size="sm"
                                  variant="outline"
                                  onClick={() => setShowCustomModelForm(true)}
                                >
                                  Добавить свою модель
                                </Button>
                              ) : (
                                <div className="space-y-3">
                                  <p className="text-sm font-medium">Своя модель (GGUF)</p>
                                  <p className="text-xs text-muted-foreground">
                                    Прямая ссылка на файл .gguf с HuggingFace или другого хостинга
                                  </p>
                                  <div className="space-y-2">
                                    <Label className="text-xs">Название</Label>
                                    <Input
                                      value={customModelName}
                                      onChange={(e) => setCustomModelName(e.target.value)}
                                      placeholder="My Model 7B"
                                    />
                                  </div>
                                  <div className="space-y-2">
                                    <Label className="text-xs">Описание (необязательно)</Label>
                                    <Input
                                      value={customModelDescription}
                                      onChange={(e) => setCustomModelDescription(e.target.value)}
                                      placeholder="Краткое описание"
                                    />
                                  </div>
                                  <div className="space-y-2">
                                    <Label className="text-xs">URL файла .gguf</Label>
                                    <Input
                                      value={customModelUrl}
                                      onChange={(e) => setCustomModelUrl(e.target.value)}
                                      placeholder="https://huggingface.co/.../resolve/main/model.gguf"
                                    />
                                  </div>
                                  <div className="flex flex-wrap gap-2">
                                    <Button
                                      size="sm"
                                      onClick={() => void handleAddCustomModel()}
                                      disabled={addingCustomModel}
                                    >
                                      {addingCustomModel && (
                                        <Loader2 className="mr-2 h-3 w-3 animate-spin" />
                                      )}
                                      Добавить
                                    </Button>
                                    <Button
                                      size="sm"
                                      variant="ghost"
                                      onClick={() => setShowCustomModelForm(false)}
                                      disabled={addingCustomModel}
                                    >
                                      Отмена
                                    </Button>
                                  </div>
                                </div>
                              )}
                            </div>

              {llmModels.length > 0 && (
                <div className="space-y-2">
                  {llmModels.map((m) => renderModelCard(m))}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
        ) : null}

        </div>
      </div>
    </div>
  );
}

function DownloadProgressBar({ value }: { value: number }) {
  const pct = Math.min(100, Math.max(0, value));
  return (
    <div className="h-2 w-full overflow-hidden rounded-full bg-secondary">
      <div
        className="h-full rounded-full bg-primary transition-all duration-300"
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

function TestButton({
  platform,
  testing,
  result,
  onTest,
  disabled,
}: {
  platform: "vk" | "telegram" | "deepseek" | "proxy";
  testing: string | null;
  result?: ApiTestResult;
  onTest: (p: "vk" | "telegram" | "deepseek" | "proxy") => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center gap-3">
      <Button
        variant="outline"
        size="sm"
        onClick={() => onTest(platform)}
        disabled={disabled || testing === platform}
      >
        {testing === platform ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <TestTube className="h-4 w-4" />
        )}
        Проверить
      </Button>
      {result && (
        <span className={`text-sm ${result.success ? "text-success" : "text-destructive"}`}>
          {result.message}
        </span>
      )}
    </div>
  );
}
