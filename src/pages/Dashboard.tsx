import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  RefreshCw,
  Newspaper,
  CheckCircle,
  Clock,
  Rss,
  Loader2,
  Activity,
  Timer,
  Send,
  History,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { PostImage } from "@/components/posts/PostImage";
import { PostPreview } from "@/components/posts/PostPreview";
import { StatusBadge } from "@/components/posts/StatusBadge";
import { getDashboardStats, fetchNews, getAutomationStatus, getRecentPublishedPosts } from "@/lib/tauri";
import type { AutomationStatus, DashboardStats, FetchResult, Post, PostStatus } from "@/lib/types";
import { formatDate, formatDuration, truncate } from "@/lib/utils";

function useCountdown(targetIso: string | null | undefined) {
  const [secondsLeft, setSecondsLeft] = useState<number | null>(null);

  useEffect(() => {
    if (!targetIso) {
      setSecondsLeft(null);
      return;
    }

    const tick = () => {
      const diff = Math.max(
        0,
        Math.floor((new Date(targetIso).getTime() - Date.now()) / 1000)
      );
      setSecondsLeft(diff);
    };

    tick();
    const timer = setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [targetIso]);

  return secondsLeft;
}

function MetricTile({
  icon: Icon,
  label,
  value,
  hint,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="rounded-lg border border-border bg-secondary/40 p-3">
      <div className="mb-2 flex items-center gap-2 text-muted-foreground">
        <Icon className="h-3.5 w-3.5 shrink-0" />
        <span className="text-xs">{label}</span>
      </div>
      <p className="text-lg font-semibold leading-tight">{value}</p>
      {hint && <p className="mt-1 text-[11px] text-muted-foreground">{hint}</p>}
    </div>
  );
}

export function Dashboard() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [automation, setAutomation] = useState<AutomationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [fetching, setFetching] = useState(false);
  const [fetchResult, setFetchResult] = useState<FetchResult | null>(null);
  const [recentPublished, setRecentPublished] = useState<Post[]>([]);

  const loadStats = async () => {
    try {
      const [data, status, published] = await Promise.all([
        getDashboardStats(),
        getAutomationStatus(),
        getRecentPublishedPosts(5),
      ]);
      setStats(data);
      setAutomation(status);
      setRecentPublished(published);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadStats();
    const timer = setInterval(loadStats, 3000);
    return () => clearInterval(timer);
  }, []);

  const handleFetch = async () => {
    setFetching(true);
    setFetchResult(null);
    try {
      const result = await fetchNews();
      setFetchResult(result);
      await loadStats();
    } catch (e) {
      setFetchResult({
        scanned_items: 0,
        new_posts: 0,
        processed_posts: 0,
        skipped_duplicates: 0,
        errors: [String(e)],
      });
    } finally {
      setFetching(false);
    }
  };

  const secondsUntilPublish = useCountdown(
    automation?.auto_publish_enabled ? automation.next_publish_at : null
  );

  const fetchActive = (stats?.sources_active ?? 0) > 0;
  const fetchStatusLabel = !automation?.auto_fetch_enabled
    ? "Выкл"
    : automation?.fetch_running
      ? "Сбор выполняется"
      : fetchActive
        ? "Работает"
        : "Нет активных источников";
  const fetchStatusTone = !automation?.auto_fetch_enabled
    ? "bg-muted text-muted-foreground border-border"
    : automation?.fetch_running
      ? "bg-warning/15 text-warning border-warning/30"
      : fetchActive
        ? "bg-success/15 text-success border-success/30"
        : "bg-muted text-muted-foreground border-border";

  const statCards = [
    { label: "Сегодня", value: stats?.posts_today ?? 0, icon: Newspaper, color: "text-primary" },
    { label: "В очереди", value: stats?.posts_pending ?? 0, icon: Clock, color: "text-warning" },
    { label: "Опубликовано", value: stats?.posts_published ?? 0, icon: CheckCircle, color: "text-success" },
    { label: "Источников", value: stats?.sources_active ?? 0, icon: Rss, color: "text-[#2AABEE]" },
  ];

  return (
    <div className="p-8">
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Дашборд</h1>
          <p className="text-muted-foreground">Обзор публикаций и сбор новостей</p>
        </div>
        <Button onClick={handleFetch} disabled={fetching}>
          {fetching ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
          Собрать новости
        </Button>
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-20">
          <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
        </div>
      ) : (
        <>
          <div className="mb-8 grid grid-cols-4 gap-4">
            {statCards.map(({ label, value, icon: Icon, color }) => (
              <Card key={label}>
                <CardContent className="flex items-center gap-4 p-6">
                  <div className={`rounded-lg bg-secondary p-3 ${color}`}>
                    <Icon className="h-5 w-5" />
                  </div>
                  <div>
                    <p className="text-2xl font-bold">{value}</p>
                    <p className="text-sm text-muted-foreground">{label}</p>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>

          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <div className="space-y-6">
            <Card>
              <CardHeader className="pb-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <CardTitle className="flex items-center gap-2">
                      <Activity className="h-5 w-5 text-primary" />
                      Автопарсинг
                    </CardTitle>
                    <CardDescription className="mt-1">
                      {automation?.last_fetch_at
                        ? `Последний сбор · ${formatDate(automation.last_fetch_at)}`
                        : "Ожидание первого сбора"}
                    </CardDescription>
                  </div>
                  <span
                    className={`inline-flex shrink-0 items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${fetchStatusTone}`}
                  >
                    <span
                      className={`h-2 w-2 rounded-full ${
                        !automation?.auto_fetch_enabled
                          ? "bg-muted-foreground"
                          : automation?.fetch_running
                            ? "animate-pulse bg-warning"
                            : fetchActive
                              ? "bg-success"
                              : "bg-muted-foreground"
                      }`}
                    />
                    {fetchStatusLabel}
                  </span>
                </div>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-3">
                  <MetricTile
                    icon={Clock}
                    label="Интервал сбора"
                    value={
                      automation?.auto_fetch_enabled
                        ? `${automation?.fetch_interval_minutes ?? 30} мин`
                        : "Выкл"
                    }
                  />
                  <MetricTile
                    icon={Rss}
                    label="Источников"
                    value={String(stats?.sources_active ?? 0)}
                    hint="активных"
                  />
                  <MetricTile
                    icon={Newspaper}
                    label="Добавлено"
                    value={String(automation?.last_fetch_new_posts ?? 0)}
                    hint="в прошлый раз"
                  />
                  {automation?.auto_publish_enabled ? (
                    <MetricTile
                      icon={Send}
                      label="Автопубликация"
                      value={`${automation.auto_publish_interval_minutes} мин + ${Math.min(automation.auto_publish_jitter_seconds_min, automation.auto_publish_jitter_seconds_max)}–${Math.max(automation.auto_publish_jitter_seconds_min, automation.auto_publish_jitter_seconds_max)} с`}
                      hint={`очередь: ${automation.queue_size}`}
                    />
                  ) : (
                    <MetricTile
                      icon={Send}
                      label="Автопубликация"
                      value="Выкл"
                      hint="включите в настройках"
                    />
                  )}
                </div>

                {automation?.auto_publish_enabled && (
                  <p className="text-xs text-muted-foreground">
                    Сначала публикуются самые свежие посты из очереди
                  </p>
                )}

                {automation && automation.last_fetch_errors.length > 0 && !automation.fetch_running && (
                  <div className="rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive">
                    {automation.last_fetch_errors.slice(0, 3).map((err, i) => (
                      <p key={i} className="truncate">
                        {err}
                      </p>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>

            {fetchResult && (
              <Card>
                <CardHeader>
                  <CardTitle>Результат сбора</CardTitle>
                  <CardDescription>Только что выполненный ручной сбор</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="grid grid-cols-2 gap-3">
                    <MetricTile
                      icon={RefreshCw}
                      label="Проверено"
                      value={String(fetchResult.scanned_items)}
                      hint="записей RSS"
                    />
                    <MetricTile
                      icon={Newspaper}
                      label="Новых"
                      value={String(fetchResult.new_posts)}
                      hint="постов"
                    />
                    <MetricTile
                      icon={Activity}
                      label="AI"
                      value={String(fetchResult.processed_posts)}
                      hint="обработано"
                    />
                    {fetchResult.skipped_duplicates > 0 && (
                      <MetricTile
                        icon={CheckCircle}
                        label="Дублей"
                        value={String(fetchResult.skipped_duplicates)}
                        hint="пропущено"
                      />
                    )}
                  </div>
                  {fetchResult.errors.length > 0 && (
                    <div className="mt-4 rounded-lg border border-destructive/20 bg-destructive/10 p-3 text-sm text-destructive">
                      {fetchResult.errors.map((err, i) => (
                        <p key={i}>{err}</p>
                      ))}
                    </div>
                  )}
                  {fetchResult.new_posts > 0 && (
                    <Link to="/posts">
                      <Button variant="outline" size="sm" className="mt-4">
                        Перейти к постам
                      </Button>
                    </Link>
                  )}
                </CardContent>
              </Card>
            )}

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <History className="h-5 w-5 text-success" />
                    Последние публикации
                  </CardTitle>
                  <CardDescription>5 последних постов в VK и Telegram</CardDescription>
                </div>
                <Link to="/history">
                  <Button variant="outline" size="sm">
                    Вся история
                  </Button>
                </Link>
              </CardHeader>
              <CardContent>
                {recentPublished.length === 0 ? (
                  <p className="py-6 text-center text-sm text-muted-foreground">
                    Пока нет опубликованных постов
                  </p>
                ) : (
                  <div className="space-y-2">
                    {recentPublished.map((post) => (
                      <Link
                        key={post.id}
                        to={`/posts/${post.id}`}
                        className="flex items-center gap-4 rounded-lg border border-border bg-secondary/20 p-3 transition-colors hover:bg-accent/50"
                      >
                        {post.raw_image_url ? (
                          <PostImage
                            url={post.raw_image_url}
                            alt=""
                            className="h-12 w-12 shrink-0 rounded-md object-cover"
                            onError={(e) => {
                              (e.target as HTMLImageElement).style.display = "none";
                            }}
                          />
                        ) : (
                          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-md bg-secondary text-muted-foreground">
                            <Newspaper className="h-5 w-5" />
                          </div>
                        )}
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-medium">
                            {truncate(post.ai_title || post.raw_title, 90)}
                          </p>
                          <p className="mt-0.5 text-xs text-muted-foreground">
                            {formatDate(post.published_at)}
                            {post.category_name && ` · ${post.category_name}`}
                          </p>
                        </div>
                        <div className="hidden shrink-0 gap-2 text-xs sm:flex">
                          {post.vk_post_id && (
                            <span className="text-[#0077FF]">VK</span>
                          )}
                          {post.telegram_message_id && (
                            <span className="text-[#2AABEE]">TG</span>
                          )}
                        </div>
                      </Link>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
            </div>

            <Card className="lg:sticky lg:top-6 lg:self-start">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Timer className="h-5 w-5" />
                  Очередь публикации
                </CardTitle>
                <CardDescription>
                  {automation?.next_post
                    ? "Следующий пост к публикации"
                    : "Нет готовых постов в очереди"}
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                {automation?.auto_publish_enabled && automation.scheduled_delay_seconds > 0 && (
                  <div className="flex flex-wrap gap-6 text-sm">
                    <div>
                      <p className="text-muted-foreground">До публикации</p>
                      <p className="text-2xl font-bold tabular-nums text-primary">
                        {secondsUntilPublish !== null
                          ? formatDuration(secondsUntilPublish)
                          : "—"}
                      </p>
                    </div>
                    <div>
                      <p className="text-muted-foreground">Задержка цикла (рандом)</p>
                      <p className="text-2xl font-bold tabular-nums">
                        {formatDuration(automation.scheduled_delay_seconds)}
                      </p>
                    </div>
                    {automation.next_publish_at && (
                      <div>
                        <p className="text-muted-foreground">Публикация в</p>
                        <p className="text-sm font-medium">
                          {formatDate(automation.next_publish_at)}
                        </p>
                      </div>
                    )}
                  </div>
                )}

                {automation?.next_post ? (
                  <>
                    <div className="flex flex-wrap items-center gap-3">
                      <StatusBadge status={automation.next_post.status as PostStatus} />
                      {automation.next_post.category_name && (
                        <span className="text-sm text-muted-foreground">
                          {automation.next_post.category_name}
                        </span>
                      )}
                      <Link
                        to={`/posts/${automation.next_post.id}`}
                        className="text-sm text-primary hover:underline"
                      >
                        Открыть в редакторе
                      </Link>
                    </div>
                    <PostPreview
                      platform="unified"
                      title={automation.next_post.title}
                      text={automation.next_post.text}
                      hashtags={automation.next_post.hashtags}
                      imageUrl={automation.next_post.image_url}
                    />
                  </>
                ) : (
                  <p className="text-sm text-muted-foreground">
                    Добавьте и обработайте посты, чтобы они попали в очередь автопубликации.
                  </p>
                )}
              </CardContent>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
