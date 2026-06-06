import { useEffect, useState } from "react";
import { ExternalLink, Loader2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { getPublishedPosts } from "@/lib/tauri";
import type { Post } from "@/lib/types";
import { formatDate, truncate } from "@/lib/utils";

export function History() {
  const [posts, setPosts] = useState<Post[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getPublishedPosts()
      .then(setPosts)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

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
                  <a
                    href={post.source_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-muted-foreground hover:text-primary"
                  >
                    <ExternalLink className="h-4 w-4" />
                  </a>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
