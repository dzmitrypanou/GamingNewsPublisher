import { useEffect, useState } from "react";
import { Plus, Trash2, Eye, Loader2, Check, Rss, AlertCircle } from "lucide-react";
import { EmptyState } from "@/components/ui/empty-state";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  getSources,
  getCategories,
  getPresetSources,
  addSource,
  updateSource,
  deleteSource,
  addPresetSources,
  previewSource,
} from "@/lib/tauri";
import type { Source, Category, PresetSource, RssPreviewItem } from "@/lib/types";
import { dialog } from "@/lib/dialog";
import { formatDate } from "@/lib/utils";

export function Sources() {
  const [sources, setSources] = useState<Source[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [presets, setPresets] = useState<PresetSource[]>([]);
  const [loading, setLoading] = useState(true);
  const [newUrl, setNewUrl] = useState("");
  const [newName, setNewName] = useState("");
  const [newCategoryId, setNewCategoryId] = useState<number | null>(null);
  const [selectedPresets, setSelectedPresets] = useState<Set<string>>(new Set());
  const [preview, setPreview] = useState<RssPreviewItem[] | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  const load = async () => {
    try {
      const [s, c, p] = await Promise.all([
        getSources(),
        getCategories(),
        getPresetSources(),
      ]);
      setSources(s);
      setCategories(c);
      setPresets(p);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const handleAdd = async () => {
    if (!newUrl.trim()) return;
    try {
      await addSource(newUrl.trim(), newName.trim() || newUrl.trim(), newCategoryId);
      setNewUrl("");
      setNewName("");
      await load();
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handleToggle = async (source: Source) => {
    await updateSource({ ...source, enabled: !source.enabled });
    await load();
  };

  const handleDelete = async (id: number) => {
    if (
      !(await dialog.confirm("Источник будет удалён из списка.", {
        title: "Удалить источник?",
        confirmText: "Удалить",
        destructive: true,
      }))
    ) {
      return;
    }
    await deleteSource(id);
    await load();
  };

  const handleAddPresets = async () => {
    if (selectedPresets.size === 0) return;
    try {
      const added = await addPresetSources(Array.from(selectedPresets));
      await dialog.alert(`Добавлено источников: ${added}`, {
        title: "Готово",
        variant: "success",
      });
      setSelectedPresets(new Set());
      await load();
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    }
  };

  const handlePreview = async (url: string) => {
    setPreviewLoading(true);
    setPreviewUrl(url);
    setPreview(null);
    setPreviewError(null);
    try {
      const items = await previewSource(url);
      setPreview(items);
    } catch (e) {
      setPreview(null);
      setPreviewError(String(e));
    } finally {
      setPreviewLoading(false);
    }
  };

  const togglePreset = (url: string) => {
    setSelectedPresets((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  };

  const existingUrls = new Set(sources.map((s) => s.url));

  const presetGroups = presets.reduce<Record<string, PresetSource[]>>((acc, p) => {
    const group = p.group || "Other";
    if (!acc[group]) acc[group] = [];
    acc[group].push(p);
    return acc;
  }, {});

  const groupOrder = [
    "General Gaming News",
    "Cybersport",
    "Industry & Business",
    "Leaks & Rumors",
    "Science",
  ];

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold">Источники RSS</h1>
        <p className="text-muted-foreground">Управление фидами игровых новостей</p>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Добавить источник</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="space-y-2">
                <Label>URL</Label>
                <Input
                  value={newUrl}
                  onChange={(e) => setNewUrl(e.target.value)}
                  placeholder="https://example.com/rss"
                />
              </div>
              <div className="space-y-2">
                <Label>Название</Label>
                <Input
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  placeholder="Название источника"
                />
              </div>
              <div className="space-y-2">
                <Label>Категория</Label>
                <select
                  className="flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
                  value={newCategoryId ?? ""}
                  onChange={(e) =>
                    setNewCategoryId(e.target.value ? parseInt(e.target.value) : null)
                  }
                >
                  <option value="">Без категории</option>
                  {categories.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </div>
              <Button onClick={handleAdd}>
                <Plus className="h-4 w-4" />
                Добавить
              </Button>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Предустановленные</CardTitle>
              <CardDescription>Топ мировых игровых изданий по категориям</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {groupOrder
                .filter((g) => presetGroups[g]?.length)
                .map((group) => (
                  <div key={group}>
                    <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      {group}
                    </p>
                    <div className="space-y-2">
                      {presetGroups[group].map((p) => (
                        <label
                          key={`${group}-${p.url}`}
                          className={`flex cursor-pointer items-center gap-3 rounded-md border p-3 transition-colors ${
                            existingUrls.has(p.url)
                              ? "border-success/30 bg-success/5 opacity-60"
                              : selectedPresets.has(p.url)
                              ? "border-primary bg-primary/5"
                              : "border-border hover:bg-accent"
                          }`}
                        >
                          <input
                            type="checkbox"
                            checked={selectedPresets.has(p.url)}
                            disabled={existingUrls.has(p.url)}
                            onChange={() => togglePreset(p.url)}
                            className="accent-primary"
                          />
                          <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium">{p.name}</p>
                            <p className="text-xs text-muted-foreground">{p.category_name}</p>
                          </div>
                          {existingUrls.has(p.url) && (
                            <Check className="h-4 w-4 shrink-0 text-success" />
                          )}
                        </label>
                      ))}
                    </div>
                  </div>
                ))}
              <Button
                variant="outline"
                onClick={handleAddPresets}
                disabled={selectedPresets.size === 0}
                className="w-full"
              >
                Добавить выбранные ({selectedPresets.size})
              </Button>
            </CardContent>
          </Card>
        </div>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Активные источники ({sources.length})</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              {sources.length === 0 ? (
                <EmptyState
                  icon={<Rss className="h-5 w-5" />}
                  title="Нет источников"
                  description="Добавьте RSS-ленту вручную или выберите из предустановленных"
                />
              ) : (
                sources.map((s) => (
                  <div
                    key={s.id}
                    className="flex items-center gap-3 rounded-md border border-border p-3"
                  >
                    <Switch
                      checked={s.enabled}
                      onCheckedChange={() => handleToggle(s)}
                    />
                    <div className="flex-1 min-w-0">
                      <p className="truncate text-sm font-medium">{s.name}</p>
                      <p className="truncate text-xs text-muted-foreground">{s.url}</p>
                      {s.last_fetched_at && (
                        <p className="text-xs text-muted-foreground">
                          Обновлён: {formatDate(s.last_fetched_at)}
                        </p>
                      )}
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => handlePreview(s.url)}
                    >
                      <Eye className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => handleDelete(s.id)}
                    >
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </div>
                ))
              )}
            </CardContent>
          </Card>

          {(preview !== null || previewLoading || previewError) && (
            <Card>
              <CardHeader>
                <CardTitle>Предпросмотр</CardTitle>
                <CardDescription className="truncate">{previewUrl}</CardDescription>
              </CardHeader>
              <CardContent>
                {previewLoading ? (
                  <div className="flex justify-center py-12">
                    <Loader2 className="h-7 w-7 animate-spin text-muted-foreground" />
                  </div>
                ) : previewError ? (
                  <EmptyState
                    variant="error"
                    icon={<AlertCircle className="h-5 w-5" />}
                    title="Не удалось загрузить ленту"
                    description={previewError}
                  />
                ) : preview && preview.length > 0 ? (
                  <div className="space-y-3">
                    {preview.map((item, i) => (
                      <div key={i} className="rounded-md border border-border p-3">
                        <p className="text-sm font-medium">{item.title}</p>
                        <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                          {item.description}
                        </p>
                      </div>
                    ))}
                  </div>
                ) : (
                  <EmptyState
                    icon={<Rss className="h-5 w-5" />}
                    title="Записей не найдено"
                    description="RSS-лента пуста или не содержит записей для предпросмотра"
                  />
                )}
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
