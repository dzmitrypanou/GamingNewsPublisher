import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Loader2, RefreshCw, Trash2, Trash } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PostImage } from "@/components/posts/PostImage";
import { StatusBadge } from "@/components/posts/StatusBadge";
import { getPosts, deletePost, deleteQueuePosts } from "@/lib/tauri";
import type { Post, PostStatus } from "@/lib/types";
import { dialog } from "@/lib/dialog";
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
  const [deletingAll, setDeletingAll] = useState(false);
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
    if (
      !(await dialog.confirm("Пост будет удалён без возможности восстановления.", {
        title: "Удалить пост?",
        confirmText: "Удалить",
        destructive: true,
      }))
    ) {
      return;
    }
    await deletePost(id);
    await load();
  };

  const handleDeleteAll = async () => {
    if (
      !(await dialog.confirm(
        "Будут удалены все посты из очереди (новые, AI, одобренные, ошибки). Опубликованные посты не затрагиваются.",
        {
          title: "Удалить всю очередь?",
          confirmText: "Удалить всё",
          destructive: true,
        }
      ))
    ) {
      return;
    }
    setDeletingAll(true);
    try {
      const deleted = await deleteQueuePosts();
      await dialog.alert(`Удалено постов: ${deleted}`, {
        title: "Готово",
        variant: "success",
      });
      await load();
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setDeletingAll(false);
    }
  };

  return (
    <div className="p-8">
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Очередь постов</h1>
          <p className="text-muted-foreground">
            Новости для публикации в VK и Telegram. При автопубликации сначала уходят самые свежие.
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            className="text-destructive hover:text-destructive"
            onClick={handleDeleteAll}
            disabled={deletingAll}
          >
            {deletingAll ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Trash className="h-4 w-4" />
            )}
            Удалить все
          </Button>
          <Button variant="outline" onClick={load}>
            <RefreshCw className="h-4 w-4" />
            Обновить
          </Button>
        </div>
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
                    <PostImage
                      url={post.raw_image_url}
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
