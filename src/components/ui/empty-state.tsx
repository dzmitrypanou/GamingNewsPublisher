import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  description?: string;
  variant?: "default" | "error";
  className?: string;
}

export function EmptyState({
  icon,
  title,
  description,
  variant = "default",
  className,
}: EmptyStateProps) {
  const isError = variant === "error";

  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center px-4 py-12 text-center",
        className
      )}
    >
      <div
        className={cn(
          "mb-4 flex h-12 w-12 items-center justify-center rounded-full border",
          isError
            ? "border-destructive/30 bg-destructive/10 text-destructive"
            : "border-border bg-secondary/50 text-muted-foreground"
        )}
      >
        {icon}
      </div>
      <p className={cn("text-sm font-medium", isError ? "text-destructive" : "text-foreground")}>
        {title}
      </p>
      {description && (
        <p className="mt-1.5 max-w-sm text-xs leading-relaxed text-muted-foreground">
          {description}
        </p>
      )}
    </div>
  );
}
