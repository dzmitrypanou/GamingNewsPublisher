import { useEffect, useState } from "react";
import { Loader2, Save, TestTube } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  getSettings,
  saveSettings,
  testVk,
  testTelegram,
  testDeepseek,
} from "@/lib/tauri";
import type { AppSettings, ApiTestResult } from "@/lib/types";

const DEFAULT_PROMPT = `Перепиши игровую новость для соцсетей VK и Telegram.
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
  fetch_interval_minutes: 30,
  auto_publish: false,
  auto_ai_process: true,
  post_language: "ru",
};

export function Settings() {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testResults, setTestResults] = useState<Record<string, ApiTestResult>>({});
  const [testing, setTesting] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

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

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveSettings(settings);
      setSaved(true);
    } catch (e) {
      alert(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async (platform: "vk" | "telegram" | "deepseek") => {
    setTesting(platform);
    try {
      await saveSettings(settings);
      const fn = platform === "vk" ? testVk : platform === "telegram" ? testTelegram : testDeepseek;
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

      <div className="space-y-6">
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
                Переменные: {"{title}"}, {"{description}"}, {"{category}"}
              </p>
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
            <CardTitle>Общие</CardTitle>
            <CardDescription>Автоматизация и интервалы</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label>Интервал автопарсинга (мин)</Label>
              <Input
                type="number"
                min={5}
                max={1440}
                value={settings.fetch_interval_minutes}
                onChange={(e) => update("fetch_interval_minutes", parseInt(e.target.value) || 30)}
              />
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
                <Label>Автопубликация</Label>
                <p className="text-xs text-muted-foreground">Публиковать в VK и Telegram без подтверждения</p>
              </div>
              <Switch
                checked={settings.auto_publish}
                onCheckedChange={(v) => update("auto_publish", v)}
              />
            </div>
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
  platform: "vk" | "telegram" | "deepseek";
  testing: string | null;
  result?: ApiTestResult;
  onTest: (p: "vk" | "telegram" | "deepseek") => void;
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
