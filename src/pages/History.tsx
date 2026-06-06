import { useEffect, useState } from "react";
import { ExternalLink, Loader2, Undo2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { getPublishedPosts, unpublishPost } from "@/lib/tauri";
import type { Post } from "@/lib/types";
import { dialog } from "@/lib/dialog";
import { formatDate, truncate } from "@/lib/utils";

export function History() {
  const [posts, setPosts] = useState<Post[]>([]);
  const [loading, setLoading] = useState(true);
  const [unpublishingId, setUnpublishingId] = useState<number | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      const data = await getPublishedPosts();
      setPosts(data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const handleUnpublish = async (id: number) => {
    if (
      !(await dialog.confirm("Пост будет удалён из VK и Telegram.", {
        title: "Снять с публикации?",
        confirmText: "Удалить",
        destructive: true,
      }))
    ) {
      return;
    }
    setUnpublishingId(id);
    try {
      const result = await unpublishPost(id);
      if (!result.vk_success || !result.telegram_success) {
        await dialog.alert(
          `VK: ${result.vk_message}\nTelegram: ${result.telegram_message}`,
          { title: "Частичная ошибка", variant: "error" }
        );
      }
      await load();
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setUnpublishingId(null);
    }
  };

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold">История</h1>
        <p className="text-muted-foreground">Опубликованные посты в VK и Telegram</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Опубликовано ({posts.length})</CardTitle>
        </CardHeader>
        <CardContent>
          {loading ? (
            <div className="flex justify-center py-10">
              <Loader2 className="h-6 w-6 animate-spin" />
            </div>
          ) : posts.length === 0 ? (
            <p className="py-10 text-center text-muted-foreground">
              Пока нет опубликованных постов
            </p>
          ) : (
            <div className="space-y-3">
              {posts.map((post) => (
                <div
                  key={post.id}
                  className="flex items-start gap-4 rounded-md border border-border p-4"
                >
                  {post.raw_image_url && (
                    <img
                      src={post.raw_image_url}
                      alt=""
                      className="h-14 w-14 rounded object-cover"
                      onError={(e) => {
                        (e.target as HTMLImageElement).style.display = "none";
                      }}
                    />
                  )}
                  <div className="flex-1 min-w-0">
                    <p className="font-medium">
                      {truncate(post.ai_title || post.raw_title, 100)}
                    </p>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {formatDate(post.published_at)}
                      {post.category_name && ` · ${post.category_name}`}
                    </p>
                    <div className="mt-2 flex gap-3 text-xs">
                      {post.vk_post_id && (
                        <span className="text-[#0077FF]">
                          VK: post #{post.vk_post_id}
                        </span>
                      )}
                      {post.telegram_message_id && (
                        <span className="text-[#2AABEE]">
                          TG: msg #{post.telegram_message_id}
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      className="text-destructive hover:text-destructive"
                      onClick={() => handleUnpublish(post.id)}
                      disabled={unpublishingId === post.id}
                    >
                      {unpublishingId === post.id ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : (
                        <Undo2 className="h-4 w-4" />
                      )}
                      Отменить
                    </Button>
                    <a
                      href={post.source_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-muted-foreground hover:text-primary"
                    >
                      <ExternalLink className="h-4 w-4" />
                    </a>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
