import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AlertCircle, AlertTriangle, CheckCircle2, Info } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  subscribeDialog,
  type DialogRequest,
  type DialogVariant,
} from "@/lib/dialog";
import { cn } from "@/lib/utils";

const variantStyles: Record<
  DialogVariant,
  { icon: typeof Info; iconClass: string; accentClass: string }
> = {
  info: {
    icon: Info,
    iconClass: "text-primary",
    accentClass: "border-primary/30 bg-primary/10",
  },
  success: {
    icon: CheckCircle2,
    iconClass: "text-success",
    accentClass: "border-success/30 bg-success/10",
  },
  error: {
    icon: AlertCircle,
    iconClass: "text-destructive",
    accentClass: "border-destructive/30 bg-destructive/10",
  },
};

export function DialogProvider({ children }: { children: ReactNode }) {
  const [request, setRequest] = useState<DialogRequest | null>(null);

  useEffect(() => subscribeDialog(setRequest), []);

  useEffect(() => {
    if (!request) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (request.kind === "confirm") {
          request.resolve(false);
          setRequest(null);
        } else {
          request.resolve();
          setRequest(null);
        }
      }
      if (event.key === "Enter" && request.kind === "confirm") {
        request.resolve(true);
        setRequest(null);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [request]);

  const closeAlert = () => {
    if (request?.kind === "alert") {
      request.resolve();
      setRequest(null);
    }
  };

  const closeConfirm = (value: boolean) => {
    if (request?.kind === "confirm") {
      request.resolve(value);
      setRequest(null);
    }
  };

  const style = request ? variantStyles[request.variant] : null;
  const Icon = style?.icon ?? Info;

  const overlay =
    request && style ? (
      <div
        className="fixed inset-0 z-[9999] flex items-center justify-center p-6"
        role="presentation"
        onClick={() => {
          if (request.kind === "alert") closeAlert();
          else closeConfirm(false);
        }}
      >
        <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" />
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="app-dialog-title"
          className="relative w-full max-w-sm rounded-2xl border border-border bg-card px-6 py-8 shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex flex-col items-center text-center">
            <div
              className={cn(
                "mb-5 flex h-14 w-14 items-center justify-center rounded-full border",
                style.accentClass
              )}
            >
              <Icon className={cn("h-7 w-7", style.iconClass)} />
            </div>

            <h2 id="app-dialog-title" className="text-xl font-semibold leading-tight">
              {request.title}
            </h2>
            <p className="mt-3 max-w-xs whitespace-pre-wrap text-sm leading-relaxed text-muted-foreground">
              {request.message}
            </p>

            <div
              className={cn(
                "mt-8 flex w-full gap-2",
                request.kind === "confirm" ? "flex-col-reverse sm:flex-row" : "justify-center"
              )}
            >
              {request.kind === "confirm" && (
                <Button
                  variant="outline"
                  className="w-full sm:flex-1"
                  onClick={() => closeConfirm(false)}
                >
                  {request.cancelText}
                </Button>
              )}
              <Button
                className={request.kind === "confirm" ? "w-full sm:flex-1" : "min-w-[120px]"}
                variant={
                  request.kind === "confirm" && request.destructive
                    ? "destructive"
                    : "default"
                }
                onClick={() => {
                  if (request.kind === "alert") closeAlert();
                  else closeConfirm(true);
                }}
              >
                {request.kind === "alert" ? (
                  "OK"
                ) : request.destructive ? (
                  <span className="inline-flex items-center gap-2">
                    <AlertTriangle className="h-4 w-4" />
                    {request.confirmText}
                  </span>
                ) : (
                  request.confirmText
                )}
              </Button>
            </div>
          </div>
        </div>
      </div>
    ) : null;

  return (
    <>
      {children}
      {overlay && createPortal(overlay, document.body)}
    </>
  );
}
