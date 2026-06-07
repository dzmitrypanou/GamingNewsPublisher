import { useEffect, useMemo, useState } from "react";
import { FileUp, Link2, Loader2, Save, TestTube, Trash2 } from "lucide-react";
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
} from "@/lib/tauri";
import { dialog } from "@/lib/dialog";
import type { AppSettings, ApiTestResult } from "@/lib/types";
import { cn, countProxyLines, mergeProxyLists } from "@/lib/utils";

const DEFAULT_PROMPT = `Переведи игровую новость на {language} и перепиши для соцсетей VK и Telegram.
Все поля ответа строго на {language}.
Формат ответа JSON:
{
  "title": "короткий цепляющий заголовок (до 80 символов)",
  "text": "2-4 предложения, понятно и без воды (до 500 символов)",
  "hashtags": ["#игры", "#название_игры"]
}
Исходные данные: {title}, {description}, категория: {category}`;

const defaultSettings: AppSettings = {
  vk_token: "",
  vk_group_id: "",
  telegram_bot_token: "",
  telegram_channel_id: "",
  deepseek_api_key: "",
  deepseek_model: "deepseek-chat",
  ai_prompt_template: DEFAULT_PROMPT,
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
  post_language: "ru",
  proxy_enabled: false,
  proxy_type: "http",
  proxy_list: "",
};

export function Settings() {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testResults, setTestResults] = useState<Record<string, ApiTestResult>>({});
  const [testing, setTesting] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [proxyUrl, setProxyUrl] = useState("");
  const [proxyImporting, setProxyImporting] = useState<"file" | "url" | null>(null);

  const proxyCount = useMemo(() => countProxyLines(settings.proxy_list), [settings.proxy_list]);

  useEffect(() => {
    getSettings()
      .then((s) => setSettings({ ...defaultSettings, ...s }))
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const update = (key: keyof AppSettings, value: string | number | boolean) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
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
    try {
      await saveSettings(settings);
      setSaved(true);
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
    <div className="p-8">
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Настройки</h1>
          <p className="text-muted-foreground">API-ключи и параметры приложения</p>
        </div>
        <Button onClick={handleSave} disabled={saving}>
          {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
          {saved ? "Сохранено" : "Сохранить"}
        </Button>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-[#0077FF]">VKontakte</CardTitle>
            <CardDescription>Токен сообщества и ID группы для публикации на стену</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label>Access Token</Label>
              <Input
                type="password"
                value={settings.vk_token}
                onChange={(e) => update("vk_token", e.target.value)}
                placeholder="vk1.a...."
              />
            </div>
            <div className="space-y-2">
              <Label>ID группы</Label>
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
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-[#2AABEE]">Telegram</CardTitle>
            <CardDescription>Бот и канал для публикации новостей</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label>Bot Token</Label>
              <Input
                type="password"
                value={settings.telegram_bot_token}
                onChange={(e) => update("telegram_bot_token", e.target.value)}
                placeholder="123456:ABC..."
              />
            </div>
            <div className="space-y-2">
              <Label>Channel ID</Label>
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
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>DeepSeek AI</CardTitle>
            <CardDescription>Генерация заголовков и текста постов</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label>API Key</Label>
              <Input
                type="password"
                value={settings.deepseek_api_key}
                onChange={(e) => update("deepseek_api_key", e.target.value)}
                placeholder="sk-..."
              />
            </div>
            <div className="space-y-2">
              <Label>Модель</Label>
              <Input
                value={settings.deepseek_model}
                onChange={(e) => update("deepseek_model", e.target.value)}
                placeholder="deepseek-chat"
              />
            </div>
            <div className="space-y-2">
              <Label>Шаблон промпта</Label>
              <Textarea
                value={settings.ai_prompt_template}
                onChange={(e) => update("ai_prompt_template", e.target.value)}
                rows={8}
                className="font-mono text-xs"
              />
              <p className="text-xs text-muted-foreground">
                Переменные: {"{title}"}, {"{description}"}, {"{category}"}, {"{language}"}
              </p>
            </div>
            <div className="flex items-center justify-between rounded-lg border border-border p-4">
              <div>
                <Label>Проверка дублей с помощью AI</Label>
                <p className="text-xs text-muted-foreground">
                  При сборе и публикации дубли определяет только DeepSeek. Без этой
                  опции семантические дубли не проверяются.
                </p>
                {!settings.deepseek_api_key && (
                  <p className="mt-1 text-xs text-warning">Укажите API ключ DeepSeek</p>
                )}
              </div>
              <Switch
                checked={settings.ai_duplicate_check}
                disabled={!settings.deepseek_api_key}
                onCheckedChange={(v) => update("ai_duplicate_check", v)}
              />
            </div>
            <TestButton
              platform="deepseek"
              testing={testing}
              result={testResults.deepseek}
              onTest={handleTest}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Прокси</CardTitle>
            <CardDescription>
              HTTP, HTTPS или SOCKS5 — для RSS, API и публикации. По одному прокси на строку.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
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

                <div className="space-y-3 rounded-lg border border-border bg-secondary/10 p-4">
                  <Label>Импорт списка</Label>
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
                    rows={8}
                    className="min-h-[180px] resize-y rounded-none border-0 bg-background font-mono text-xs leading-relaxed focus-visible:ring-0 focus-visible:ring-offset-0"
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

        <Card>
          <CardHeader>
            <CardTitle>Общие</CardTitle>
            <CardDescription>Автоматизация и интервалы</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
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
                За один сбор проверяется: источники × это число (например, 10 × 10 = 100 записей)
              </p>
            </div>
            <div className="flex items-center justify-between">
              <div>
                <Label>Автообработка AI</Label>
                <p className="text-xs text-muted-foreground">Переписывать новости через DeepSeek при сборе</p>
              </div>
              <Switch
                checked={settings.auto_ai_process}
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
    </div>
  );
}

function TestButton({
  platform,
  testing,
  result,
  onTest,
}: {
  platform: "vk" | "telegram" | "deepseek" | "proxy";
  testing: string | null;
  result?: ApiTestResult;
  onTest: (p: "vk" | "telegram" | "deepseek" | "proxy") => void;
}) {
  return (
    <div className="flex items-center gap-3">
      <Button
        variant="outline"
        size="sm"
        onClick={() => onTest(platform)}
        disabled={testing === platform}
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
