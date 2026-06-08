import { useEffect, useMemo, useRef, useState } from "react";
import { Calendar, ChevronLeft, ChevronRight, Clock, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

const WEEKDAYS = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];

const MONTHS = [
  "Январь",
  "Февраль",
  "Март",
  "Апрель",
  "Май",
  "Июнь",
  "Июль",
  "Август",
  "Сентябрь",
  "Октябрь",
  "Ноябрь",
  "Декабрь",
];

export function parseScheduleDateTime(value: string): { date: string; time: string } | null {
  const match = value.match(/^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/);
  if (!match) return null;
  return { date: match[1], time: match[2] };
}

function formatDisplayValue(value: string): string {
  const parsed = parseScheduleDateTime(value);
  if (!parsed) return "";
  const date = new Date(`${parsed.date}T${parsed.time}:00`);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString("ru-RU", {
    day: "numeric",
    month: "long",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function toDateString(year: number, month: number, day: number): string {
  return `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

function getCalendarDays(year: number, month: number) {
  const firstDay = new Date(year, month, 1);
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const startOffset = (firstDay.getDay() + 6) % 7;
  const cells: Array<{ date: string | null; day: number }> = [];

  for (let i = 0; i < startOffset; i++) {
    cells.push({ date: null, day: 0 });
  }
  for (let day = 1; day <= daysInMonth; day++) {
    cells.push({ date: toDateString(year, month, day), day });
  }
  return cells;
}

interface ScheduleDateTimePickerProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  defaultTime?: string;
  className?: string;
}

export function ScheduleDateTimePicker({
  value,
  onChange,
  placeholder = "Выберите дату и время",
  defaultTime = "09:00",
  className,
}: ScheduleDateTimePickerProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const parsed = parseScheduleDateTime(value);
  const today = new Date();
  const initialView = parsed
    ? {
        year: Number(parsed.date.slice(0, 4)),
        month: Number(parsed.date.slice(5, 7)) - 1,
      }
    : { year: today.getFullYear(), month: today.getMonth() };

  const [open, setOpen] = useState(false);
  const [viewYear, setViewYear] = useState(initialView.year);
  const [viewMonth, setViewMonth] = useState(initialView.month);
  const [draftDate, setDraftDate] = useState(parsed?.date ?? "");
  const [draftTime, setDraftTime] = useState(parsed?.time ?? defaultTime);

  useEffect(() => {
    const next = parseScheduleDateTime(value);
    setDraftDate(next?.date ?? "");
    setDraftTime(next?.time ?? defaultTime);
    if (next) {
      setViewYear(Number(next.date.slice(0, 4)));
      setViewMonth(Number(next.date.slice(5, 7)) - 1);
    }
  }, [value, defaultTime]);

  useEffect(() => {
    if (!open) return;

    const onPointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };

    window.addEventListener("mousedown", onPointerDown);
    return () => window.removeEventListener("mousedown", onPointerDown);
  }, [open]);

  const calendarDays = useMemo(
    () => getCalendarDays(viewYear, viewMonth),
    [viewYear, viewMonth]
  );

  const displayValue = formatDisplayValue(value);
  const todayStr = toDateString(today.getFullYear(), today.getMonth(), today.getDate());

  const applyDraft = (date: string, time: string) => {
    if (!date || !time) {
      onChange("");
      return;
    }
    onChange(`${date}T${time}`);
  };

  const shiftMonth = (delta: number) => {
    const next = new Date(viewYear, viewMonth + delta, 1);
    setViewYear(next.getFullYear());
    setViewMonth(next.getMonth());
  };

  const selectDay = (date: string) => {
    setDraftDate(date);
    applyDraft(date, draftTime);
  };

  const updateTime = (time: string) => {
    setDraftTime(time);
    const date = draftDate || todayStr;
    if (!draftDate) setDraftDate(date);
    applyDraft(date, time);
  };

  const clearValue = () => {
    setDraftDate("");
    setDraftTime(defaultTime);
    onChange("");
    setOpen(false);
  };

  return (
    <div ref={rootRef} className={cn("relative", className)}>
      <Button
        type="button"
        variant="outline"
        onClick={() => setOpen((prev) => !prev)}
        className="h-10 w-full justify-start gap-2 px-3 font-normal"
      >
        <Calendar className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className={cn("truncate", !displayValue && "text-muted-foreground")}>
          {displayValue || placeholder}
        </span>
      </Button>

      {open && (
        <div className="absolute left-0 top-[calc(100%+0.5rem)] z-50 w-[min(100vw-2rem,20rem)] rounded-lg border border-border bg-card p-3 shadow-lg">
          <div className="mb-3 flex items-center justify-between gap-2">
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => shiftMonth(-1)}
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>
            <p className="text-sm font-medium">
              {MONTHS[viewMonth]} {viewYear}
            </p>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => shiftMonth(1)}
            >
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>

          <div className="mb-1 grid grid-cols-7 gap-1">
            {WEEKDAYS.map((day) => (
              <div
                key={day}
                className="flex h-8 items-center justify-center text-[11px] font-medium text-muted-foreground"
              >
                {day}
              </div>
            ))}
          </div>

          <div className="grid grid-cols-7 gap-1">
            {calendarDays.map((cell, index) =>
              cell.date ? (
                <button
                  key={cell.date}
                  type="button"
                  onClick={() => selectDay(cell.date!)}
                  className={cn(
                    "flex h-8 w-full items-center justify-center rounded-md text-sm transition-colors",
                    draftDate === cell.date
                      ? "bg-primary text-primary-foreground"
                      : cell.date === todayStr
                        ? "border border-primary/40 text-foreground hover:bg-accent"
                        : "text-foreground hover:bg-accent"
                  )}
                >
                  {cell.day}
                </button>
              ) : (
                <div key={`empty-${index}`} className="h-8" />
              )
            )}
          </div>

          <div className="mt-3 space-y-2 border-t border-border pt-3">
            <div className="flex items-center gap-2">
              <Clock className="h-4 w-4 text-muted-foreground" />
              <LabelLike>Время</LabelLike>
            </div>
            <Input
              type="time"
              value={draftTime}
              onChange={(e) => updateTime(e.target.value)}
              className="w-full"
            />
          </div>

          <div className="mt-3 flex justify-between gap-2">
            <Button type="button" variant="ghost" size="sm" onClick={clearValue}>
              <X className="h-3.5 w-3.5" />
              Сбросить
            </Button>
            <Button type="button" size="sm" onClick={() => setOpen(false)}>
              Готово
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

function LabelLike({ children }: { children: React.ReactNode }) {
  return <span className="text-xs font-medium text-muted-foreground">{children}</span>;
}
