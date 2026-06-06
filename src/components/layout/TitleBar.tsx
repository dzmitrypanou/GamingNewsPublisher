import { useEffect, useState, type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, Copy, X } from "lucide-react";
import { cn } from "@/lib/utils";

const PAGE_TITLES: Record<string, string> = {
  "/": "Дашборд",
  "/posts": "Посты",
  "/sources": "Источники",
  "/categories": "Категории",
  "/history": "История",
  "/duplicates": "Дубли",
  "/settings": "Настройки",
};

function getPageTitle(pathname: string) {
  if (pathname.startsWith("/posts/") && pathname !== "/posts") {
    return "Редактор поста";
  }
  return PAGE_TITLES[pathname] ?? "Gaming News Publisher";
}

export function TitleBar() {
  const { pathname } = useLocation();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const win = getCurrentWindow();

    win.isMaximized().then((value) => {
      if (!cancelled) setMaximized(value);
    });

    const unlistenPromise = win.onResized(() => {
      win.isMaximized().then((value) => {
        if (!cancelled) setMaximized(value);
      });
    });

    return () => {
      cancelled = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize();
  const handleToggleMaximize = () => getCurrentWindow().toggleMaximize();
  const handleClose = () => getCurrentWindow().close();

  return (
    <header className="flex h-[30px] shrink-0 items-stretch border-b border-border bg-background select-none">
      <div
        data-tauri-drag-region
        className="flex min-w-0 flex-1 items-center px-4 text-xs font-semibold tracking-wide text-muted-foreground uppercase"
      >
        {getPageTitle(pathname)}
      </div>
      <div className="flex items-stretch">
        <WindowButton onClick={handleMinimize} label="Свернуть">
          <Minus className="h-3.5 w-3.5" />
        </WindowButton>
        <WindowButton onClick={handleToggleMaximize} label={maximized ? "Восстановить" : "Развернуть"}>
          {maximized ? <Copy className="h-3 w-3" /> : <Square className="h-3 w-3" />}
        </WindowButton>
        <WindowButton onClick={handleClose} label="Закрыть" close>
          <X className="h-3.5 w-3.5" />
        </WindowButton>
      </div>
    </header>
  );
}

function WindowButton({
  onClick,
  label,
  children,
  close,
}: {
  onClick: () => void;
  label: string;
  children: ReactNode;
  close?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "inline-flex w-[46px] items-center justify-center text-muted-foreground transition-colors",
        close
          ? "hover:bg-destructive hover:text-white"
          : "hover:bg-accent hover:text-foreground"
      )}
    >
      {children}
    </button>
  );
}
