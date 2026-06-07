import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowDown, ExternalLink, Loader2, Newspaper, Settings } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { PostImage } from "@/components/posts/PostImage";
import { StatusBadge } from "@/components/posts/StatusBadge";
import { getDuplicatesOverview } from "@/lib/tauri";
import type { DuplicateGroup, DuplicateRecord, DuplicatesOverview, PostStatus } from "@/lib/types";
import { formatDate, truncate } from "@/lib/utils";

const REASON_LABELS: Record<string, string> = {
  ai_duplicate: "AI-дубль",
  already_parsed: "Уже обработан",
  insert_skip: "Похожая новость",
  db_removed: "Удалён из базы",
};

function reasonLabel(reason: string) {
  return REASON_LABELS[reason] ?? reason;
}

function AiVerdict({ record }: { record: DuplicateRecord }) {
  if (record.ai_checked_at == null || record.ai_is_duplicate == null) {
    return (
      <p className="text-xs text-muted-foreground">AI: не проверено</p>
    );
  }

  const confidence = record.ai_confidence ?? 0;
  const isDup = record.ai_is_duplicate;

  return (
    <div
      className={`rounded-lg border p-3 text-left ${
        isDup
          ? "border-warning/30 bg-warning/10"
          : "border-success/30 bg-success/10"
      }`}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant={isDup ? "warning" : "success"}>
          {isDup ? "Дубль" : "Разные новости"}
        </Badge>
        <span className="text-xs text-muted-foreground">уверенность {confidence}%</span>
      </div>
      {record.ai_explanation && (
        <p className="mt-2 text-xs leading-relaxed text-foreground">
          {record.ai_explanation}
        </p>
      )}
    </div>
  );
}

function KeptPostCard({ post }: { post: NonNullable<DuplicateGroup["kept_post"]> }) {
  const title = post.ai_title || post.raw_title;
  const text = post.ai_text || post.raw_description;

  return (
    <Link
      to={`/posts/${post.id}`}
      className="block rounded-lg border-2 border-success/40 bg-success/5 p-4 transition-colors hover:bg-success/10"
    >
      <div className="mb-2 flex items-center gap-2">
        <Badge variant="success">Остался</Badge>
        <StatusBadge status={post.status as PostStatus} />
      </div>
      <div className="flex gap-3">
        {post.raw_image_url ? (
          <PostImage
            url={post.raw_image_url}
            alt=""
            className="h-14 w-14 shrink-0 rounded-md object-cover"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
        ) : (
          <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-md bg-secondary text-muted-foreground">
            <Newspaper className="h-5 w-5" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <p className="font-medium leading-snug">{truncate(title, 120)}</p>
          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
            {truncate(text, 160)}
          </p>
          <p className="mt-1.5 text-xs text-muted-foreground">{formatDate(post.created_at)}</p>
        </div>
      </div>
    </Link>
  );
}

function DuplicateItem({ record }: { record: DuplicateRecord }) {
  return (
    <div className="relative ml-6 border-l-2 border-warning/40 pl-4">
      <div className="absolute -left-[9px] top-5 h-3 w-3 rounded-full border-2 border-warning bg-background" />
      <div className="rounded-lg border border-warning/30 bg-warning/5 p-3">
        <div className="flex flex-wrap items-start justify-between gap-2">
          <p className="min-w-0 flex-1 text-sm font-medium leading-snug">
            {truncate(record.duplicate_title, 120)}
          </p>
          <Badge variant="outline" className="shrink-0 text-xs">
            {reasonLabel(record.reason)}
          </Badge>
        </div>
        {record.duplicate_description && (
          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
            {truncate(record.duplicate_description, 160)}
          </p>
        )}
        <p className="mt-1 text-xs text-muted-foreground">{formatDate(record.created_at)}</p>

        <div className="mt-3 space-y-2">
          <AiVerdict record={record} />
          <a
            href={record.duplicate_url}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
          >
            <ExternalLink className="h-3 w-3" />
            Источник дубля
          </a>
        </div>
      </div>
    </div>
  );
}

function DuplicateGroupCard({ group }: { group: DuplicateGroup }) {
  const keptPost = group.kept_post;

  return (
    <Card>
      <CardHeader className="pb-3">
        <div>
          <CardTitle className="text-base">
            {keptPost
              ? "Связь: оригинал → дубли"
              : "Дубли без привязки к посту"}
          </CardTitle>
          <CardDescription>
            {group.duplicates.length}{" "}
            {group.duplicates.length === 1 ? "дубль" : "дублей"}
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {keptPost ? (
          <>
            <KeptPostCard post={keptPost} />
            <div className="flex items-center gap-2 px-2 text-xs text-muted-foreground">
              <ArrowDown className="h-4 w-4 shrink-0 text-warning" />
              <span>Отфильтровано как дубль</span>
            </div>
          </>
        ) : (
          <p className="rounded-lg border border-dashed border-border p-3 text-sm text-muted-foreground">
            Оригинальный пост не найден в базе — связь только по журналу
          </p>
        )}

        <div className="space-y-3">
          {group.duplicates.map((record) => (
            <DuplicateItem key={record.id} record={record} />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

export function Duplicates() {
  const [data, setData] = useState<DuplicatesOverview | null>(null);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    try {
      const overview = await getDuplicatesOverview();
      setData(overview);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const aiEnabled = data?.ai_duplicate_check_enabled ?? false;

  return (
    <div className="p-8">
      <div className="mb-8">
        <h1 className="text-2xl font-bold">Дубли</h1>
        <p className="text-muted-foreground">
          {aiEnabled
            ? "Журнал дублей, определённых DeepSeek при сборе и публикации"
            : "Семантическая проверка дублей отключена"}
        </p>
      </div>

      {loading ? (
        <div className="flex justify-center py-20">
          <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
      ) : !aiEnabled ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-12 text-center">
            <Settings className="h-10 w-10 text-muted-foreground" />
            <p className="text-sm text-muted-foreground">
              Включите проверку дублей AI в настройках
            </p>
            <Link
              to="/settings"
              className="text-sm text-primary hover:underline"
            >
              Перейти в настройки
            </Link>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-6">
          {!data || data.groups.length === 0 ? (
            <Card>
              <CardContent className="py-12 text-center text-sm text-muted-foreground">
                Дублей пока не зафиксировано
              </CardContent>
            </Card>
          ) : (
            data.groups.map((group) => (
              <DuplicateGroupCard
                key={`${group.kept_post_id ?? "orphan"}-${group.duplicates[0]?.id ?? 0}`}
                group={group}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}
