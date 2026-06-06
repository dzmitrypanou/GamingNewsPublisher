import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, Loader2, Sparkles, Send, Undo2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PostPreview } from "@/components/posts/PostPreview";
import { StatusBadge } from "@/components/posts/StatusBadge";
import { getPost, updatePost, processPostWithAi, publishPost, unpublishPost } from "@/lib/tauri";
import { dialog } from "@/lib/dialog";
import type { Post, PublishResult, UnpublishResult } from "@/lib/types";

export function PostEditor() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [post, setPost] = useState<Post | null>(null);
  const [loading, setLoading] = useState(true);
  const [aiLoading, setAiLoading] = useState(false);
  const [publishing, setPublishing] = useState(false);
  const [unpublishing, setUnpublishing] = useState(false);
  const [publishResult, setPublishResult] = useState<PublishResult | null>(null);
  const [unpublishResult, setUnpublishResult] = useState<UnpublishResult | null>(null);

  const [title, setTitle] = useState("");
  const [text, setText] = useState("");
  const [hashtags, setHashtags] = useState("");

  useEffect(() => {
    if (!id) return;
    getPost(parseInt(id))
      .then((p) => {
        setPost(p);
        setTitle(p.ai_title || p.raw_title);
        setText(p.ai_text || p.raw_description);
        setHashtags(p.ai_hashtags || "");
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [id]);

  const handleSave = async () => {
    if (!post) return;
    const updated = {
      ...post,
      ai_title: title,
      ai_text: text,
      ai_hashtags: hashtags,
      status: "approved" as const,
    };
    await updatePost(updated);
    setPost(updated);
  };

  const handleAi = async () => {
    if (!post) return;
    setAiLoading(true);
    try {
      const updated = await processPostWithAi(post.id);
      setPost(updated);
      setTitle(updated.ai_title || updated.raw_title);
      setText(updated.ai_text || updated.raw_description);
      setHashtags(updated.ai_hashtags || "");
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка AI", variant: "error" });
    } finally {
      setAiLoading(false);
    }
  };

  const canUnpublish =
    post?.status === "published" ||
    (post?.status === "failed" && !!(post.vk_post_id || post.telegram_message_id));

  const handleUnpublish = async () => {
    if (!post) return;
    if (
      !(await dialog.confirm(
        "Пост будет удалён из VK и Telegram и вернётся в очередь.",
        {
          title: "Снять с публикации?",
          confirmText: "Удалить",
          destructive: true,
        }
      ))
    ) {
      return;
    }
    setUnpublishing(true);
    setUnpublishResult(null);
    try {
      const result = await unpublishPost(post.id);
      setUnpublishResult(result);
      const refreshed = await getPost(post.id);
      setPost(refreshed);
      setPublishResult(null);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка", variant: "error" });
    } finally {
      setUnpublishing(false);
    }
  };

  const handlePublish = async () => {
    if (!post) return;
    setPublishing(true);
    setPublishResult(null);
    try {
      await handleSave();
      const result = await publishPost(post.id);
      setPublishResult(result);
      const refreshed = await getPost(post.id);
      setPost(refreshed);
    } catch (e) {
      await dialog.alert(String(e), { title: "Ошибка публикации", variant: "error" });
    } finally {
      setPublishing(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (!post) {
    return <div className="p-8">Пост не найден</div>;
  }

  return (
    <div className="p-8">
      <div className="mb-6 flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate("/posts")}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-xl font-bold">Редактор поста</h1>
          <p className="text-sm text-muted-foreground truncate">{post.source_url}</p>
        </div>
        <StatusBadge status={post.status} />
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Редактирование</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label>Заголовок</Label>
                <Input value={title} onChange={(e) => setTitle(e.target.value)} />
              </div>
              <div className="space-y-2">
                <Label>Текст</Label>
                <Textarea
                  value={text}
                  onChange={(e) => setText(e.target.value)}
                  rows={6}
                />
              </div>
              <div className="space-y-2">
                <Label>Хештеги</Label>
                <Input
                  value={hashtags}
                  onChange={(e) => setHashtags(e.target.value)}
                  placeholder="#игры #новости"
                />
              </div>

              {post.raw_image_url && (
                <div className="space-y-2">
                  <Label>Изображение</Label>
                  <img
                    src={post.raw_image_url}
                    alt=""
                    className="max-h-40 rounded-md object-cover"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                    }}
                  />
                </div>
              )}

              <div className="flex flex-wrap gap-2">
                <Button variant="outline" onClick={handleAi} disabled={aiLoading}>
                  {aiLoading ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Sparkles className="h-4 w-4" />
                  )}
                  Перегенерировать AI
                </Button>
                <Button variant="outline" onClick={handleSave}>
                  Сохранить
                </Button>
                <Button onClick={handlePublish} disabled={publishing || post.status === "published"}>
                  {publishing ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Send className="h-4 w-4" />
                  )}
                  Опубликовать
                </Button>
                {canUnpublish && (
                  <Button
                    variant="outline"
                    className="text-destructive hover:text-destructive"
                    onClick={handleUnpublish}
                    disabled={unpublishing}
                  >
                    {unpublishing ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Undo2 className="h-4 w-4" />
                    )}
                    Отменить публикацию
                  </Button>
                )}
              </div>

              {unpublishResult && (
                <div className="space-y-1 text-sm">
                  <p className={unpublishResult.vk_success ? "text-success" : "text-destructive"}>
                    VK: {unpublishResult.vk_message}
                  </p>
                  <p className={unpublishResult.telegram_success ? "text-success" : "text-destructive"}>
                    Telegram: {unpublishResult.telegram_message}
                  </p>
                </div>
              )}

              {publishResult && (
                <div className="space-y-1 text-sm">
                  <p className={publishResult.vk_success ? "text-success" : "text-destructive"}>
                    VK: {publishResult.vk_message}
                  </p>
                  <p className={publishResult.telegram_success ? "text-success" : "text-destructive"}>
                    Telegram: {publishResult.telegram_message}
                  </p>
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-sm">Исходная новость</CardTitle>
            </CardHeader>
            <CardContent className="text-sm text-muted-foreground">
              <p className="font-medium text-foreground">{post.raw_title}</p>
              <p className="mt-2 line-clamp-4">{post.raw_description}</p>
            </CardContent>
          </Card>
        </div>

        <div className="space-y-4">
          <PostPreview
            platform="vk"
            title={title}
            text={text}
            hashtags={hashtags}
            imageUrl={post.raw_image_url}
          />
          <PostPreview
            platform="telegram"
            title={title}
            text={text}
            hashtags={hashtags}
            imageUrl={post.raw_image_url}
          />
        </div>
      </div>
    </div>
  );
}
