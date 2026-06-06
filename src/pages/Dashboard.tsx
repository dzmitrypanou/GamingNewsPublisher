import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { RefreshCw, Newspaper, CheckCircle, Clock, Rss, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { getDashboardStats, fetchNews } from "@/lib/tauri";
import type { DashboardStats, FetchResult } from "@/lib/types";
import { formatDate } from "@/lib/utils";

export function Dashboard() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [fetching, setFetching] = useState(false);
  const [fetchResult, setFetchResult] = useState<FetchResult | null>(null);

  const loadStats = async () => {
    try {
      const data = await getDashboardStats();
      setStats(data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadStats();
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
        new_posts: 0,
        processed_posts: 0,
        errors: [String(e)],
      });
    } finally {
      setFetching(false);
    }
  };

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

          <div className="grid grid-cols-2 gap-6">
            <Card>
              <CardHeader>
                <CardTitle>Последний сбор</CardTitle>
                <CardDescription>
                  {stats?.last_fetch_at
                    ? formatDate(stats.last_fetch_at)
                    : "Ещё не выполнялся"}
                </CardDescription>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-muted-foreground">
                  Настройте источники RSS и API-ключи в{" "}
                  <Link to="/settings" className="text-primary hover:underline">
                    настройках
                  </Link>
                  , затем нажмите «Собрать новости».
                </p>
              </CardContent>
            </Card>

            {fetchResult && (
              <Card>
                <CardHeader>
                  <CardTitle>Результат сбора</CardTitle>
                </CardHeader>
                <CardContent className="space-y-2 text-sm">
                  <p>Новых постов: <strong>{fetchResult.new_posts}</strong></p>
                  <p>Обработано AI: <strong>{fetchResult.processed_posts}</strong></p>
                  {fetchResult.errors.length > 0 && (
                    <div className="mt-2 rounded-md bg-destructive/10 p-3 text-destructive">
                      {fetchResult.errors.map((err, i) => (
                        <p key={i}>{err}</p>
                      ))}
                    </div>
                  )}
                  {fetchResult.new_posts > 0 && (
                    <Link to="/posts">
                      <Button variant="outline" size="sm" className="mt-2">
                        Перейти к постам
                      </Button>
                    </Link>
                  )}
                </CardContent>
              </Card>
            )}
          </div>
        </>
      )}
    </div>
  );
}
