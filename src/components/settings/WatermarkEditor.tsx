import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, FileUp, Loader2, Move } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  getWatermarkNaturalSize,
  pickWatermarkFile,
  readLocalImageDataUrl,
} from "@/lib/tauri";
import type { AppSettings, WatermarkBackdrop, WatermarkPreset, WatermarkPreviewBackground } from "@/lib/types";
import {
  RESIZE_HANDLES,
  WATERMARK_BACKDROPS,
  WATERMARK_PRESETS,
  WATERMARK_PREVIEW_BACKGROUNDS,
  applyWatermarkResize,
  backdropLogoMoveRange,
  clampBackdropLogoOffset,
  getPreviewBackgroundStyle,
  getWatermarkBackdropLayers,
  resolveWatermarkPlacement,
  scalePercentFromWidth,
  type ResizeHandle,
  type WatermarkLayoutInput,
} from "@/lib/watermark";
import { cn } from "@/lib/utils";

interface WatermarkEditorProps {
  settings: AppSettings;
  onChange: (key: keyof AppSettings, value: string | number | boolean) => void;
  onPatch: (patch: Partial<AppSettings>) => void;
}

export function WatermarkEditor({ settings, onChange, onPatch }: WatermarkEditorProps) {
  const previewRef = useRef<HTMLDivElement>(null);
  const previewWrapRef = useRef<HTMLDivElement>(null);
  const [picking, setPicking] = useState(false);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [naturalSize, setNaturalSize] = useState({ width: 200, height: 80 });
  const [previewContainerWidth, setPreviewContainerWidth] = useState(560);
  const [previewBackground, setPreviewBackground] = useState<WatermarkPreviewBackground>("white");
  const [dragging, setDragging] = useState(false);
  const [resizing, setResizing] = useState<ResizeHandle | null>(null);
  const dragOffset = useRef({ x: 0, y: 0 });
  const resizeStart = useRef({ x: 0, y: 0, w: 0, h: 0, mouseX: 0, mouseY: 0 });

  const layoutInput: WatermarkLayoutInput = useMemo(
    () => ({
      canvasWidth: settings.post_image_width,
      canvasHeight: settings.post_image_height,
      watermarkWidth: naturalSize.width,
      watermarkHeight: naturalSize.height,
      sizeMode: settings.watermark_size_mode,
      customWidth: settings.watermark_width_px,
      customHeight: settings.watermark_height_px,
      scalePercent: settings.watermark_scale_percent,
      positionMode: settings.watermark_position_mode,
      preset: settings.watermark_preset,
      marginX: settings.watermark_margin_x,
      marginY: settings.watermark_margin_y,
      x: settings.watermark_x,
      y: settings.watermark_y,
    }),
    [settings, naturalSize]
  );

  const watermarkLayout = useMemo(
    () =>
      resolveWatermarkPlacement(
        layoutInput,
        settings.watermark_backdrop,
        settings.watermark_backdrop_padding,
        settings.watermark_backdrop_logo_x,
        settings.watermark_backdrop_logo_y
      ),
    [
      layoutInput,
      settings.watermark_backdrop,
      settings.watermark_backdrop_padding,
      settings.watermark_backdrop_logo_x,
      settings.watermark_backdrop_logo_y,
    ]
  );
  const placement = watermarkLayout.logo;

  const previewScale =
    Math.min(560, previewContainerWidth) / settings.post_image_width;
  const previewDisplayWidth = settings.post_image_width * previewScale;
  const previewDisplayHeight = settings.post_image_height * previewScale;
  const aspect = naturalSize.width / Math.max(1, naturalSize.height);

  useEffect(() => {
    const el = previewWrapRef.current;
    if (!el) return;

    const updateWidth = () => {
      const w = el.clientWidth;
      if (w > 0) setPreviewContainerWidth(w);
    };

    updateWidth();
    const ro = new ResizeObserver(updateWidth);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    if (!settings.watermark_image) {
      setPreviewUrl(null);
      return;
    }

    Promise.all([
      readLocalImageDataUrl(settings.watermark_image),
      getWatermarkNaturalSize(settings.watermark_image),
    ])
      .then(([url, size]) => {
        setPreviewUrl(url);
        setNaturalSize({
          width: size.width || 200,
          height: size.height || 80,
        });
      })
      .catch(() => setPreviewUrl(null));
  }, [settings.watermark_image]);

  useEffect(() => {
    if (!dragging) return;

    const onMove = (event: MouseEvent) => {
      const rect = previewRef.current?.getBoundingClientRect();
      if (!rect) return;

      const localX =
        (event.clientX - rect.left) / previewScale - dragOffset.current.x;
      const localY =
        (event.clientY - rect.top) / previewScale - dragOffset.current.y;
      const maxX = Math.max(0, settings.post_image_width - placement.width);
      const maxY = Math.max(0, settings.post_image_height - placement.height);

      onPatch({
        watermark_position_mode: "manual",
        watermark_x: Math.round(Math.min(Math.max(0, localX), maxX)),
        watermark_y: Math.round(Math.min(Math.max(0, localY), maxY)),
      });
    };

    const onUp = () => setDragging(false);

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [
    dragging,
    onPatch,
    placement.height,
    placement.width,
    previewScale,
    settings.post_image_height,
    settings.post_image_width,
  ]);

  useEffect(() => {
    if (!resizing) return;

    const onMove = (event: MouseEvent) => {
      const rect = previewRef.current?.getBoundingClientRect();
      if (!rect) return;

      const dx = (event.clientX - resizeStart.current.mouseX) / previewScale;
      const dy = (event.clientY - resizeStart.current.mouseY) / previewScale;
      const next = applyWatermarkResize(
        resizing,
        {
          x: resizeStart.current.x,
          y: resizeStart.current.y,
          w: resizeStart.current.w,
          h: resizeStart.current.h,
        },
        dx,
        dy,
        aspect,
        settings.post_image_width,
        settings.post_image_height
      );

      onPatch({
        watermark_size_mode: "custom",
        watermark_width_px: next.width,
        watermark_height_px: next.height,
        watermark_position_mode: "manual",
        watermark_x: next.x,
        watermark_y: next.y,
      });
    };

    const onUp = () => setResizing(null);

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [
    aspect,
    onPatch,
    previewScale,
    resizing,
    settings.post_image_height,
    settings.post_image_width,
  ]);

  const handlePickWatermark = async () => {
    setPicking(true);
    try {
      const localRef = await pickWatermarkFile();
      onPatch({
        watermark_image: localRef,
        watermark_enabled: true,
        watermark_size_mode: "scale",
        watermark_width_px: 0,
        watermark_height_px: 0,
      });
    } catch (e) {
      const message = String(e);
      if (!message.includes("не выбран")) {
        console.error(e);
      }
    } finally {
      setPicking(false);
    }
  };

  const handlePresetSelect = (preset: WatermarkPreset) => {
    const next = resolveWatermarkPlacement(
      { ...layoutInput, preset, positionMode: "preset" },
      settings.watermark_backdrop,
      settings.watermark_backdrop_padding,
      settings.watermark_backdrop_logo_x,
      settings.watermark_backdrop_logo_y
    );
    onPatch({
      watermark_preset: preset,
      watermark_position_mode: "preset",
      watermark_x: next.logo.x,
      watermark_y: next.logo.y,
    });
  };

  const moveLogoInBackdrop = (dx: number, dy: number) => {
    const pad = settings.watermark_backdrop_padding;
    const logoX = clampBackdropLogoOffset(
      settings.watermark_backdrop_logo_x + dx,
      pad
    );
    const logoY = clampBackdropLogoOffset(
      settings.watermark_backdrop_logo_y + dy,
      pad
    );
    const next = resolveWatermarkPlacement(
      layoutInput,
      settings.watermark_backdrop,
      pad,
      logoX,
      logoY
    );
    onPatch({
      watermark_backdrop_logo_x: logoX,
      watermark_backdrop_logo_y: logoY,
      watermark_x: next.logo.x,
      watermark_y: next.logo.y,
    });
  };

  const handleManualCoord = (key: "watermark_x" | "watermark_y", raw: string) => {
    const max =
      key === "watermark_x"
        ? Math.max(0, settings.post_image_width - placement.width)
        : Math.max(0, settings.post_image_height - placement.height);
    const value = Math.min(max, Math.max(0, parseInt(raw) || 0));
    onPatch({
      watermark_position_mode: "manual",
      [key]: value,
    });
  };

  const handleScaleChange = (scalePercent: number) => {
    const next = resolveWatermarkPlacement(
      {
        ...layoutInput,
        sizeMode: "scale",
        scalePercent,
        customWidth: 0,
        customHeight: 0,
      },
      settings.watermark_backdrop,
      settings.watermark_backdrop_padding,
      settings.watermark_backdrop_logo_x,
      settings.watermark_backdrop_logo_y
    );
    onPatch({
      watermark_size_mode: "scale",
      watermark_scale_percent: scalePercent,
      watermark_width_px: 0,
      watermark_height_px: 0,
      watermark_x: next.logo.x,
      watermark_y: next.logo.y,
    });
  };

  const startDrag = (event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const rect = previewRef.current?.getBoundingClientRect();
    if (!rect) return;

    const wmLeft = placement.x * previewScale;
    const wmTop = placement.y * previewScale;
    dragOffset.current = {
      x: (event.clientX - rect.left - wmLeft) / previewScale,
      y: (event.clientY - rect.top - wmTop) / previewScale,
    };
    onChange("watermark_position_mode", "manual");
    setDragging(true);
  };

  const startResize = (handle: ResizeHandle, event: React.MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    resizeStart.current = {
      x: placement.x,
      y: placement.y,
      w: placement.width,
      h: placement.height,
      mouseX: event.clientX,
      mouseY: event.clientY,
    };
    setResizing(handle);
  };

  const sizeLabel =
    settings.watermark_size_mode === "custom"
      ? `${placement.width}×${placement.height} px`
      : `${settings.watermark_scale_percent}% ширины`;

  const backdropLayers = useMemo(
    () =>
      getWatermarkBackdropLayers({
        backdrop: settings.watermark_backdrop,
        backdropOpacity: settings.watermark_backdrop_opacity,
        backdropPadding: settings.watermark_backdrop_padding,
        backdropColor: settings.watermark_backdrop_color,
        placement,
        backdropBox: watermarkLayout.backdrop,
        canvasWidth: settings.post_image_width,
        canvasHeight: settings.post_image_height,
        scale: previewScale,
        preset: settings.watermark_preset,
        positionMode: settings.watermark_position_mode,
      }),
    [settings, placement, previewScale, watermarkLayout.backdrop]
  );

  const selectedBackdrop = WATERMARK_BACKDROPS.find((b) => b.id === settings.watermark_backdrop);
  const backdropUsesPadding = selectedBackdrop?.tiedToPosition && settings.watermark_backdrop !== "none";

  const miniPreviewInput = (backdrop: WatermarkBackdrop) => {
    const miniLayout = resolveWatermarkPlacement(
      {
        canvasWidth: 80,
        canvasHeight: 44,
        watermarkWidth: 36,
        watermarkHeight: 14,
        sizeMode: "custom",
        customWidth: 36,
        customHeight: 14,
        scalePercent: 18,
        positionMode: "preset",
        preset: settings.watermark_preset,
        marginX: 4,
        marginY: 4,
        x: 0,
        y: 0,
      },
      backdrop,
      4,
      settings.watermark_backdrop_logo_x,
      settings.watermark_backdrop_logo_y
    );
    return {
      backdrop,
      backdropColor: settings.watermark_backdrop_color,
      backdropOpacity: settings.watermark_backdrop_opacity,
      backdropPadding: 4,
      placement: miniLayout.logo,
      backdropBox: miniLayout.backdrop,
      canvasWidth: 80,
      canvasHeight: 44,
      scale: 1,
      preset: settings.watermark_preset,
      positionMode: settings.watermark_position_mode,
    };
  };

  const tiedBackdrops = WATERMARK_BACKDROPS.filter((item) => item.tiedToPosition);
  const frameBackdrops = WATERMARK_BACKDROPS.filter((item) => !item.tiedToPosition);

  const renderBackdropChip = (item: (typeof WATERMARK_BACKDROPS)[number]) => {
    const layers = getWatermarkBackdropLayers(miniPreviewInput(item.id));
    const active = settings.watermark_backdrop === item.id;
    const mini = miniPreviewInput(item.id);

    return (
      <button
        key={item.id}
        type="button"
        title={item.hint}
        onClick={() => onChange("watermark_backdrop", item.id)}
        className={cn(
          "overflow-hidden rounded-md border text-left transition-colors",
          active
            ? "border-primary ring-1 ring-primary/40"
            : "border-border hover:border-primary/50"
        )}
      >
        <div
          className="relative h-9 w-full"
          style={getPreviewBackgroundStyle("white")}
        >
          {layers.map((layer, index) => (
            <div key={index} className="pointer-events-none absolute" style={layer} />
          ))}
          <div
            className="absolute text-[7px] font-bold tracking-wide text-white"
            style={{
              left: mini.placement.x,
              top: mini.placement.y,
              width: mini.placement.width,
              height: mini.placement.height,
              lineHeight: `${mini.placement.height}px`,
              textAlign: "center",
              textShadow: "0 0 2px rgba(0,0,0,0.35)",
            }}
          >
            LOGO
          </div>
        </div>
        <p className="truncate border-t border-border px-1 py-0.5 text-center text-[9px] font-medium leading-tight">
          {item.label}
        </p>
      </button>
    );
  };

  return (
    <div className="min-w-0 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <Label>Водяной знак</Label>
          <p className="text-xs text-muted-foreground">
            PNG, JPG или SVG с прозрачным фоном
          </p>
        </div>
        <Switch
          checked={settings.watermark_enabled}
          onCheckedChange={(value) => onChange("watermark_enabled", value)}
        />
      </div>

      {settings.watermark_enabled && (
        <>
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={() => void handlePickWatermark()}
          disabled={picking}
        >
          {picking ? <Loader2 className="h-4 w-4 animate-spin" /> : <FileUp className="h-4 w-4" />}
          Выбрать файл
        </Button>
        {settings.watermark_image && (
          <span className="self-center text-xs text-muted-foreground">
            {settings.watermark_image.replace("local:", "")}
          </span>
        )}
      </div>

      {settings.watermark_image && (
        <>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <Label>Прозрачность ({settings.watermark_opacity}%)</Label>
              <input
                type="range"
                min={0}
                max={100}
                value={settings.watermark_opacity}
                onChange={(e) => onChange("watermark_opacity", parseInt(e.target.value) || 0)}
                className="w-full accent-primary"
              />
            </div>
            <div className="space-y-2">
              <Label>Размер ({sizeLabel})</Label>
              <input
                type="range"
                min={5}
                max={80}
                value={
                  settings.watermark_size_mode === "custom"
                    ? scalePercentFromWidth(settings.post_image_width, placement.width)
                    : settings.watermark_scale_percent
                }
                onChange={(e) =>
                  handleScaleChange(parseInt(e.target.value) || settings.watermark_scale_percent)
                }
                className="w-full accent-primary"
              />
            </div>
          </div>

          <div className="space-y-2">
            <Label>Подложка</Label>

            <div className="space-y-1.5">
              <p className="text-[10px] text-muted-foreground">У знака</p>
              <div className="grid grid-cols-5 gap-1.5 sm:grid-cols-6 lg:grid-cols-7">
                {tiedBackdrops.map(renderBackdropChip)}
              </div>
            </div>

            <div className="space-y-1.5">
              <p className="text-[10px] text-muted-foreground">На весь кадр</p>
              <div className="grid grid-cols-4 gap-1.5 sm:grid-cols-6 lg:grid-cols-8">
                {frameBackdrops.map(renderBackdropChip)}
              </div>
            </div>

            {selectedBackdrop && (
              <p className="text-[10px] text-muted-foreground">{selectedBackdrop.hint}</p>
            )}

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label>Непрозрачность подложки ({settings.watermark_backdrop_opacity}%)</Label>
                <input
                  type="range"
                  min={0}
                  max={100}
                  disabled={settings.watermark_backdrop === "none"}
                  value={settings.watermark_backdrop_opacity}
                  onChange={(e) =>
                    onChange("watermark_backdrop_opacity", parseInt(e.target.value) || 0)
                  }
                  className="w-full accent-primary disabled:opacity-40"
                />
              </div>
              <div className="space-y-2">
                <Label>Отступ подложки ({settings.watermark_backdrop_padding}px)</Label>
                <input
                  type="range"
                  min={0}
                  max={48}
                  disabled={!backdropUsesPadding}
                  value={settings.watermark_backdrop_padding}
                  onChange={(e) => {
                    const padding = parseInt(e.target.value) || 0;
                    const logoX = clampBackdropLogoOffset(
                      settings.watermark_backdrop_logo_x,
                      padding
                    );
                    const logoY = clampBackdropLogoOffset(
                      settings.watermark_backdrop_logo_y,
                      padding
                    );
                    const next = resolveWatermarkPlacement(
                      layoutInput,
                      settings.watermark_backdrop,
                      padding,
                      logoX,
                      logoY
                    );
                    onPatch({
                      watermark_backdrop_padding: padding,
                      watermark_backdrop_logo_x: logoX,
                      watermark_backdrop_logo_y: logoY,
                      watermark_x: next.logo.x,
                      watermark_y: next.logo.y,
                    });
                  }}
                  className="w-full accent-primary disabled:opacity-40"
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label>Цвет подложки</Label>
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  disabled={settings.watermark_backdrop === "none"}
                  value={
                    /^#[0-9a-fA-F]{6}$/.test(settings.watermark_backdrop_color)
                      ? settings.watermark_backdrop_color
                      : "#000000"
                  }
                  onChange={(e) => onChange("watermark_backdrop_color", e.target.value)}
                  className="h-9 w-14 cursor-pointer rounded border border-border bg-transparent p-1 disabled:opacity-40"
                />
                <Input
                  disabled={settings.watermark_backdrop === "none"}
                  value={settings.watermark_backdrop_color}
                  onChange={(e) => onChange("watermark_backdrop_color", e.target.value)}
                  className="font-mono text-sm disabled:opacity-40"
                  placeholder="#000000"
                />
              </div>
            </div>

            {backdropUsesPadding && (
              <div className="space-y-2">
                <Label>Позиция лого в подложке</Label>
                <p className="text-xs text-muted-foreground">
                  Смещение внутри подложки по стрелкам (0–
                  {backdropLogoMoveRange(settings.watermark_backdrop_padding)} px)
                </p>
                <div className="flex flex-wrap items-center gap-4">
                  <div className="grid grid-cols-3 gap-1">
                    <div />
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      className="h-9 w-9"
                      onClick={() => moveLogoInBackdrop(0, -1)}
                      title="Вверх"
                    >
                      <ArrowUp className="h-4 w-4" />
                    </Button>
                    <div />
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      className="h-9 w-9"
                      onClick={() => moveLogoInBackdrop(-1, 0)}
                      title="Влево"
                    >
                      <ArrowLeft className="h-4 w-4" />
                    </Button>
                    <div className="flex h-9 w-9 items-center justify-center rounded-md border border-border bg-secondary/30 text-[10px] font-medium text-muted-foreground">
                      ◎
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      className="h-9 w-9"
                      onClick={() => moveLogoInBackdrop(1, 0)}
                      title="Вправо"
                    >
                      <ArrowRight className="h-4 w-4" />
                    </Button>
                    <div />
                    <Button
                      type="button"
                      variant="outline"
                      size="icon"
                      className="h-9 w-9"
                      onClick={() => moveLogoInBackdrop(0, 1)}
                      title="Вниз"
                    >
                      <ArrowDown className="h-4 w-4" />
                    </Button>
                    <div />
                  </div>
                  <div className="text-sm text-muted-foreground">
                    X: {settings.watermark_backdrop_logo_x} px · Y:{" "}
                    {settings.watermark_backdrop_logo_y} px
                  </div>
                </div>
              </div>
            )}
          </div>

          <div className="space-y-2">
            <Label>Режим позиционирования</Label>
            <div className="grid grid-cols-2 gap-2 rounded-lg border border-border bg-secondary/20 p-1">
              {(
                [
                  ["preset", "По шаблону"],
                  ["manual", "Вручную"],
                ] as const
              ).map(([value, label]) => (
                <button
                  key={value}
                  type="button"
                  onClick={() => {
                    if (value === "preset") {
                      const next = resolveWatermarkPlacement(
                        { ...layoutInput, positionMode: "preset" },
                        settings.watermark_backdrop,
                        settings.watermark_backdrop_padding,
                        settings.watermark_backdrop_logo_x,
                        settings.watermark_backdrop_logo_y
                      );
                      onPatch({
                        watermark_position_mode: "preset",
                        watermark_x: next.logo.x,
                        watermark_y: next.logo.y,
                      });
                    } else {
                      onChange("watermark_position_mode", value);
                    }
                  }}
                  className={cn(
                    "rounded-md px-3 py-2 text-sm font-medium transition-colors",
                    settings.watermark_position_mode === value
                      ? "bg-primary text-primary-foreground shadow-sm"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground"
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {settings.watermark_position_mode === "preset" ? (
            <div className="space-y-3">
              <Label>Позиция на холсте</Label>
              <div className="grid w-fit grid-cols-3 gap-1 rounded-lg border border-border p-2">
                {WATERMARK_PRESETS.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    title={item.id}
                    onClick={() => handlePresetSelect(item.id)}
                    className={cn(
                      "flex h-10 w-10 items-center justify-center rounded-md text-lg transition-colors",
                      settings.watermark_preset === item.id
                        ? "bg-primary text-primary-foreground"
                        : "bg-secondary/60 hover:bg-accent"
                    )}
                  >
                    {item.label}
                  </button>
                ))}
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <Label>Отступ X (px)</Label>
                  <Input
                    type="number"
                    min={0}
                    max={500}
                    value={settings.watermark_margin_x}
                    onChange={(e) => {
                      const marginX = Math.max(0, parseInt(e.target.value) || 0);
                      const next = resolveWatermarkPlacement(
                        { ...layoutInput, marginX, positionMode: "preset" },
                        settings.watermark_backdrop,
                        settings.watermark_backdrop_padding,
                        settings.watermark_backdrop_logo_x,
                        settings.watermark_backdrop_logo_y
                      );
                      onPatch({
                        watermark_margin_x: marginX,
                        watermark_x: next.logo.x,
                        watermark_y: next.logo.y,
                      });
                    }}
                  />
                </div>
                <div className="space-y-2">
                  <Label>Отступ Y (px)</Label>
                  <Input
                    type="number"
                    min={0}
                    max={500}
                    value={settings.watermark_margin_y}
                    onChange={(e) => {
                      const marginY = Math.max(0, parseInt(e.target.value) || 0);
                      const next = resolveWatermarkPlacement(
                        { ...layoutInput, marginY, positionMode: "preset" },
                        settings.watermark_backdrop,
                        settings.watermark_backdrop_padding,
                        settings.watermark_backdrop_logo_x,
                        settings.watermark_backdrop_logo_y
                      );
                      onPatch({
                        watermark_margin_y: marginY,
                        watermark_x: next.logo.x,
                        watermark_y: next.logo.y,
                      });
                    }}
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <div className="space-y-2">
                <Label>X (px)</Label>
                <Input
                  type="number"
                  min={0}
                  max={settings.post_image_width}
                  value={settings.watermark_x}
                  onChange={(e) => handleManualCoord("watermark_x", e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label>Y (px)</Label>
                <Input
                  type="number"
                  min={0}
                  max={settings.post_image_height}
                  value={settings.watermark_y}
                  onChange={(e) => handleManualCoord("watermark_y", e.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label>Ширина (px)</Label>
                <Input
                  type="number"
                  min={16}
                  max={settings.post_image_width}
                  value={placement.width}
                  onChange={(e) => {
                    const width = Math.min(
                      settings.post_image_width,
                      Math.max(16, parseInt(e.target.value) || placement.width)
                    );
                    const height = Math.max(
                      16,
                      Math.round(width / aspect)
                    );
                    onPatch({
                      watermark_size_mode: "custom",
                      watermark_width_px: width,
                      watermark_height_px: height,
                      watermark_position_mode: "manual",
                    });
                  }}
                />
              </div>
              <div className="space-y-2">
                <Label>Высота (px)</Label>
                <Input
                  type="number"
                  min={16}
                  max={settings.post_image_height}
                  value={placement.height}
                  onChange={(e) => {
                    const height = Math.min(
                      settings.post_image_height,
                      Math.max(16, parseInt(e.target.value) || placement.height)
                    );
                    onPatch({
                      watermark_size_mode: "custom",
                      watermark_width_px: placement.width,
                      watermark_height_px: height,
                      watermark_position_mode: "manual",
                    });
                  }}
                />
              </div>
            </div>
          )}

          <div className="min-w-0 space-y-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                <Label>Предпросмотр</Label>
                <span className="text-xs text-muted-foreground">
                  {settings.post_image_width}×{settings.post_image_height} · перетащите или тяните за
                  край
                </span>
                <Move className="h-3.5 w-3.5 text-muted-foreground" />
              </div>
              <div className="flex gap-1 rounded-lg border border-border bg-secondary/20 p-1">
                {WATERMARK_PREVIEW_BACKGROUNDS.map((bg) => (
                  <button
                    key={bg.id}
                    type="button"
                    onClick={() => setPreviewBackground(bg.id)}
                    className={cn(
                      "rounded-md px-2 py-1 text-[11px] font-medium transition-colors",
                      previewBackground === bg.id
                        ? "bg-primary text-primary-foreground"
                        : "text-muted-foreground hover:bg-accent"
                    )}
                  >
                    {bg.label}
                  </button>
                ))}
              </div>
            </div>
            <div ref={previewWrapRef} className="w-full min-w-0 max-w-full overflow-hidden">
            <div
              ref={previewRef}
              className="relative max-w-full overflow-hidden rounded-lg border border-border"
              style={{
                width: previewDisplayWidth,
                height: previewDisplayHeight,
                maxWidth: "100%",
                ...getPreviewBackgroundStyle(previewBackground),
              }}
            >
              {previewBackground === "checker" && (
                <div
                  className="absolute inset-0 bg-gradient-to-br from-slate-700/70 to-slate-900/70"
                  aria-hidden
                />
              )}
              {previewBackground === "photo" && (
                <div
                  className="absolute inset-0 opacity-80"
                  style={{
                    backgroundImage:
                      "radial-gradient(circle at 20% 20%, rgba(255,255,255,0.35), transparent 35%), radial-gradient(circle at 80% 30%, rgba(255,255,255,0.2), transparent 30%)",
                  }}
                  aria-hidden
                />
              )}
              {backdropLayers.map((layer, index) => (
                <div key={index} className="pointer-events-none absolute" style={layer} />
              ))}
              {previewUrl && (
                <div
                  className={cn(
                    "absolute z-10 select-none",
                    dragging || resizing ? "ring-2 ring-primary" : ""
                  )}
                  style={{
                    left: placement.x * previewScale,
                    top: placement.y * previewScale,
                    width: placement.width * previewScale,
                    height: placement.height * previewScale,
                    opacity: settings.watermark_opacity / 100,
                  }}
                >
                  <div
                    role="presentation"
                    onMouseDown={startDrag}
                    className="h-full w-full cursor-move"
                  >
                    <img
                      src={previewUrl}
                      alt="Водяной знак"
                      draggable={false}
                      className="pointer-events-none h-full w-full object-fill"
                    />
                  </div>
                  {RESIZE_HANDLES.map((handle) => (
                    <div
                      key={handle.id}
                      role="presentation"
                      onMouseDown={(event) => startResize(handle.id, event)}
                      className={cn(
                        "absolute z-10 h-3 w-3 rounded-sm border border-primary bg-background shadow-sm",
                        handle.className
                      )}
                      style={{ cursor: handle.cursor }}
                    />
                  ))}
                </div>
              )}
              <div className="absolute bottom-1 right-1 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white">
                {placement.x}, {placement.y} · {placement.width}×{placement.height}
              </div>
            </div>
            </div>
          </div>
        </>
      )}
        </>
      )}
    </div>
  );
}
