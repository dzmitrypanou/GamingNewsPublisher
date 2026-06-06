import { useEffect, useState } from "react";
import { Loader2, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { getCategories, updateCategory } from "@/lib/tauri";
import { dialog } from "@/lib/dialog";
import type { Category } from "@/lib/types";

export function Categories() {
  const [categories, setCategories] = useState<Category[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<number | null>(null);

  useEffect(() => {
    getCategories()
      .then(setCategories)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const update = (id: number, field: keyof Category, value: string | boolean) => {
    setCategories((prev) =>
      prev.map((c) => (c.id === id ? { ...c, [field]: value } : c))
    );
  };

  const handleSave = async (category: Category) => {
    setSaving(category.id);
    try {
      await updateCategory(category);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setSaving(null);
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
      <div className="mb-8">
        <h1 className="text-2xl font-bold">Категории</h1>
        <p className="text-muted-foreground">Хештеги и фильтры для игровых новостей</p>
      </div>

      <div className="grid grid-cols-2 gap-4">
        {categories.map((cat) => (
          <Card key={cat.id}>
            <CardHeader className="flex flex-row items-center justify-between pb-3">
              <CardTitle className="text-base">{cat.name}</CardTitle>
              <Switch
                checked={cat.enabled}
                onCheckedChange={(v) => update(cat.id, "enabled", v)}
              />
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="space-y-1">
                <Label className="text-xs">Хештеги</Label>
                <Input
                  value={cat.hashtags}
                  onChange={(e) => update(cat.id, "hashtags", e.target.value)}
                  placeholder="#игры #PC"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-xs">Ключевые слова (через запятую)</Label>
                <Input
                  value={cat.keywords}
                  onChange={(e) => update(cat.id, "keywords", e.target.value)}
                  placeholder="steam, pc, nvidia"
                />
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={() => handleSave(cat)}
                disabled={saving === cat.id}
              >
                {saving === cat.id ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <Save className="h-3 w-3" />
                )}
                Сохранить
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
