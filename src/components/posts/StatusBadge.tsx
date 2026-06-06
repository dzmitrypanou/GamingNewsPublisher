import { Badge } from "@/components/ui/badge";
import type { PostStatus } from "@/lib/types";

const statusConfig: Record<PostStatus, { label: string; variant: "default" | "secondary" | "success" | "warning" | "destructive" }> = {
  new: { label: "Новый", variant: "secondary" },
  ai_processed: { label: "AI обработан", variant: "warning" },
  approved: { label: "Одобрен", variant: "default" },
  published: { label: "Опубликован", variant: "success" },
  failed: { label: "Ошибка", variant: "destructive" },
};

export function StatusBadge({ status }: { status: PostStatus }) {
  const config = statusConfig[status] ?? statusConfig.new;
  return <Badge variant={config.variant}>{config.label}</Badge>;
}
