import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Loader2, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBadge } from "@/components/posts/StatusBadge";
import { getPosts, deletePost } from "@/lib/tauri";
import type { Post, PostStatus } from "@/lib/types";
import { formatDate, truncate } from "@/lib/utils";

const statusFilters: { value: PostStatus | "all"; label: string }[] = [
  { value: "all", label: "Все" },
  { value: "new", label: "Новые" },
  { value: "ai_processed", label: "AI" },
  { value: "approved", label: "Одобренные" },
  { value: "published", label: "Опубликованные" },
  { value: "failed", label: "Ошибки" },
];

export function Posts() {
  const [posts, setPosts] = useState<Post[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<PostStatus | "all">("all");

  const load = async () => {
    setLoading(true);
    try {
      const data = await getPosts(filter === "all" ? undefined : filter);
      setPosts(data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, [filter]);

  const handleDelete = async (id: number) => {
    if (!confirm("Удалить пост?")) return;
    await deletePost(id);
    await load();
  };

  return (
    <div className="p-8">
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Очередь постов</h1>
          <p className="text-muted-foreground">Новости для публикации в VK и Telegram</p>
        </div>
        <Button variant="outline" onClick={load}>
          <RefreshCw className="h-4 w-4" />
          Обновить
        </Button>
      </div>

      <div className="mb-4 flex gap-2">
        {statusFilters.map((f) => (
          <Button
            key={f.value}
            variant={filter === f.value ? "default" : "outline"}
            size="sm"
            onClick={() => setFilter(f.value)}
          >
            {f.label}
          </Button>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Посты ({posts.length})</CardTitle>
        </CardHeader>
        <CardContent>
          {loading ? (
            <div className="flex justify-center py-10">
              <Loader2 className="h-6 w-6 animate-spin" />
            </div>
          ) : posts.length === 0 ? (
            <p className="py-10 text-center text-muted-foreground">
              Нет постов. Соберите новости на дашборде.
            </p>
          ) : (
            <div className="space-y-2">
              {posts.map((post) => (
                <div
                  key={post.id}
                  className="flex items-center gap-4 rounded-md border border-border p-4 hover:bg-accent/50"
                >
                  {post.raw_image_url && (
                    <img
                      src={post.raw_image_url}
                      alt=""
                      className="h-12 w-12 rounded object-cover"
                      onError={(e) => {
                        (e.target as HTMLImageElement).style.display = "none";
                      }}
                    />
                  )}
                  <div className="flex-1 min-w-0">
                    <Link
                      to={`/posts/${post.id}`}
                      className="text-sm font-medium hover:text-primary"
                    >
                      {truncate(post.ai_title || post.raw_title, 80)}
                    </Link>
                    <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
                      {post.category_name && <span>{post.category_name}</span>}
                      <span>{formatDate(post.created_at)}</span>
                    </div>
                  </div>
                  <StatusBadge status={post.status} />
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => handleDelete(post.id)}
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
