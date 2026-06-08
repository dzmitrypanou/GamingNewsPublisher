import type {
  WatermarkBackdrop,
  WatermarkPositionMode,
  WatermarkPreset,
  WatermarkPreviewBackground,
  WatermarkSizeMode,
} from "./types";

export interface WatermarkLayoutInput {
  canvasWidth: number;
  canvasHeight: number;
  watermarkWidth: number;
  watermarkHeight: number;
  sizeMode: WatermarkSizeMode;
  customWidth: number;
  customHeight: number;
  scalePercent: number;
  positionMode: WatermarkPositionMode;
  preset: WatermarkPreset;
  marginX: number;
  marginY: number;
  x: number;
  y: number;
}

export type ResizeHandle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

const MIN_WATERMARK_SIZE = 16;

export function watermarkDimensions(input: WatermarkLayoutInput) {
  if (
    input.sizeMode === "custom" &&
    input.customWidth > 0 &&
    input.customHeight > 0
  ) {
    return {
      width: Math.min(Math.max(1, input.customWidth), input.canvasWidth),
      height: Math.min(Math.max(1, input.customHeight), input.canvasHeight),
    };
  }

  const targetW = Math.max(
    1,
    Math.round((input.canvasWidth * input.scalePercent) / 100)
  );
  const aspect = input.watermarkHeight / Math.max(1, input.watermarkWidth);
  const targetH = Math.max(1, Math.round(targetW * aspect));
  return {
    width: Math.min(targetW, input.canvasWidth),
    height: Math.min(targetH, input.canvasHeight),
  };
}

export function computeWatermarkPosition(input: WatermarkLayoutInput) {
  const { width: wmW, height: wmH } = watermarkDimensions(input);
  const maxX = Math.max(0, input.canvasWidth - wmW);
  const maxY = Math.max(0, input.canvasHeight - wmH);

  if (input.positionMode === "manual") {
    return {
      x: Math.min(input.x, maxX),
      y: Math.min(input.y, maxY),
      width: wmW,
      height: wmH,
    };
  }

  const mx = Math.min(input.marginX, maxX);
  const my = Math.min(input.marginY, maxY);

  let x = mx;
  let y = my;

  switch (input.preset) {
    case "top_left":
      x = mx;
      y = my;
      break;
    case "top_center":
      x = Math.floor(maxX / 2);
      y = my;
      break;
    case "top_right":
      x = Math.max(0, maxX - mx);
      y = my;
      break;
    case "center_left":
      x = mx;
      y = Math.floor(maxY / 2);
      break;
    case "center":
      x = Math.floor(maxX / 2);
      y = Math.floor(maxY / 2);
      break;
    case "center_right":
      x = Math.max(0, maxX - mx);
      y = Math.floor(maxY / 2);
      break;
    case "bottom_left":
      x = mx;
      y = Math.max(0, maxY - my);
      break;
    case "bottom_center":
      x = Math.floor(maxX / 2);
      y = Math.max(0, maxY - my);
      break;
    case "bottom_right":
      x = Math.max(0, maxX - mx);
      y = Math.max(0, maxY - my);
      break;
  }

  return { x, y, width: wmW, height: wmH };
}

export function scalePercentFromWidth(
  canvasWidth: number,
  width: number
): number {
  return Math.min(80, Math.max(5, Math.round((width / canvasWidth) * 100)));
}

export function applyWatermarkResize(
  handle: ResizeHandle,
  start: { x: number; y: number; w: number; h: number },
  dx: number,
  dy: number,
  aspect: number,
  canvasWidth: number,
  canvasHeight: number
) {
  let x = start.x;
  let y = start.y;
  let w = start.w;
  let h = start.h;

  switch (handle) {
    case "se":
      w = Math.max(MIN_WATERMARK_SIZE, start.w + dx);
      h = Math.max(MIN_WATERMARK_SIZE, Math.round(w / aspect));
      break;
    case "nw":
      w = Math.max(MIN_WATERMARK_SIZE, start.w - dx);
      h = Math.max(MIN_WATERMARK_SIZE, Math.round(w / aspect));
      x = start.x + start.w - w;
      y = start.y + start.h - h;
      break;
    case "ne":
      w = Math.max(MIN_WATERMARK_SIZE, start.w + dx);
      h = Math.max(MIN_WATERMARK_SIZE, Math.round(w / aspect));
      y = start.y + start.h - h;
      break;
    case "sw":
      w = Math.max(MIN_WATERMARK_SIZE, start.w - dx);
      h = Math.max(MIN_WATERMARK_SIZE, Math.round(w / aspect));
      x = start.x + start.w - w;
      break;
    case "e":
      w = Math.max(MIN_WATERMARK_SIZE, start.w + dx);
      h = start.h;
      break;
    case "w":
      w = Math.max(MIN_WATERMARK_SIZE, start.w - dx);
      h = start.h;
      x = start.x + start.w - w;
      break;
    case "s":
      w = start.w;
      h = Math.max(MIN_WATERMARK_SIZE, start.h + dy);
      break;
    case "n":
      w = start.w;
      h = Math.max(MIN_WATERMARK_SIZE, start.h - dy);
      y = start.y + start.h - h;
      break;
  }

  w = Math.min(w, canvasWidth);
  h = Math.min(h, canvasHeight);
  x = Math.round(Math.min(Math.max(0, x), canvasWidth - w));
  y = Math.round(Math.min(Math.max(0, y), canvasHeight - h));

  return { x, y, width: Math.round(w), height: Math.round(h) };
}

export const WATERMARK_PRESETS: Array<{ id: WatermarkPreset; label: string }> = [
  { id: "top_left", label: "↖" },
  { id: "top_center", label: "↑" },
  { id: "top_right", label: "↗" },
  { id: "center_left", label: "←" },
  { id: "center", label: "◎" },
  { id: "center_right", label: "→" },
  { id: "bottom_left", label: "↙" },
  { id: "bottom_center", label: "↓" },
  { id: "bottom_right", label: "↘" },
];

export const RESIZE_HANDLES: Array<{
  id: ResizeHandle;
  className: string;
  cursor: string;
}> = [
  { id: "nw", className: "-left-1.5 -top-1.5", cursor: "nwse-resize" },
  { id: "n", className: "left-1/2 -top-1.5 -translate-x-1/2", cursor: "ns-resize" },
  { id: "ne", className: "-right-1.5 -top-1.5", cursor: "nesw-resize" },
  { id: "e", className: "-right-1.5 top-1/2 -translate-y-1/2", cursor: "ew-resize" },
  { id: "se", className: "-right-1.5 -bottom-1.5", cursor: "nwse-resize" },
  { id: "s", className: "left-1/2 -bottom-1.5 -translate-x-1/2", cursor: "ns-resize" },
  { id: "sw", className: "-left-1.5 -bottom-1.5", cursor: "nesw-resize" },
  { id: "w", className: "-left-1.5 top-1/2 -translate-y-1/2", cursor: "ew-resize" },
];

export interface WatermarkBackdropDef {
  id: WatermarkBackdrop;
  label: string;
  tiedToPosition: boolean;
  hint: string;
}

export const WATERMARK_BACKDROPS: WatermarkBackdropDef[] = [
  { id: "none", label: "Нет", tiedToPosition: true, hint: "Без подложки" },
  { id: "dark_pill", label: "Таблетка", tiedToPosition: true, hint: "Скруглённая подложка под знаком" },
  { id: "dark_rect", label: "Прямоугольник", tiedToPosition: true, hint: "Плотная подложка под знаком" },
  { id: "shadow", label: "Тень", tiedToPosition: true, hint: "Мягкая тень за знаком" },
  { id: "dark_glow", label: "Свечение", tiedToPosition: true, hint: "Размытая подложка за знаком" },
  { id: "bottom_bar", label: "Полоса снизу", tiedToPosition: false, hint: "На всю ширину кадра" },
  { id: "top_bar", label: "Полоса сверху", tiedToPosition: false, hint: "На всю ширину кадра" },
  { id: "bottom_gradient", label: "Градиент снизу", tiedToPosition: false, hint: "Затемнение нижней части" },
  { id: "top_gradient", label: "Градиент сверху", tiedToPosition: false, hint: "Затемнение верхней части" },
  { id: "left_strip", label: "Полоса слева", tiedToPosition: false, hint: "Вертикальная подложка" },
  { id: "right_strip", label: "Полоса справа", tiedToPosition: false, hint: "Вертикальная подложка" },
  { id: "vignette", label: "Виньетка", tiedToPosition: false, hint: "Затемнение по краям кадра" },
  { id: "corner_fade", label: "Угол", tiedToPosition: false, hint: "Градиент от угла (по шаблону позиции)" },
];

export function isTiedWatermarkBackdrop(backdrop: WatermarkBackdrop): boolean {
  const def = WATERMARK_BACKDROPS.find((item) => item.id === backdrop);
  return !!def?.tiedToPosition && backdrop !== "none";
}

export function backdropLogoMoveRange(padding: number): number {
  return Math.max(0, padding * 2);
}

export function clampBackdropLogoOffset(value: number, padding: number): number {
  return Math.min(Math.max(0, Math.round(value)), backdropLogoMoveRange(padding));
}

export function defaultBackdropLogoOffset(padding: number): number {
  return clampBackdropLogoOffset(padding, padding);
}

export function migrateBackdropLogoOffset(raw: number, padding: number): number {
  const slack = backdropLogoMoveRange(padding);
  if (slack <= 0) return 0;
  if (raw <= 100 && raw % 50 === 0) {
    return clampBackdropLogoOffset(Math.round((slack * raw) / 100), padding);
  }
  return clampBackdropLogoOffset(raw, padding);
}

export interface TiedBackdropLayoutInput extends WatermarkLayoutInput {
  backdropPadding: number;
  logoOffsetX: number;
  logoOffsetY: number;
}

export interface TiedBackdropLayout {
  backdropX: number;
  backdropY: number;
  backdropW: number;
  backdropH: number;
  logoX: number;
  logoY: number;
}

function computePositionForBox(
  input: WatermarkLayoutInput,
  boxW: number,
  boxH: number
): { x: number; y: number } {
  const maxX = Math.max(0, input.canvasWidth - boxW);
  const maxY = Math.max(0, input.canvasHeight - boxH);

  if (input.positionMode === "manual") {
    return {
      x: Math.min(input.x, maxX),
      y: Math.min(input.y, maxY),
    };
  }

  const mx = Math.min(input.marginX, maxX);
  const my = Math.min(input.marginY, maxY);

  switch (input.preset) {
    case "top_left":
      return { x: mx, y: my };
    case "top_center":
      return { x: Math.floor(maxX / 2), y: my };
    case "top_right":
      return { x: Math.max(0, maxX - mx), y: my };
    case "center_left":
      return { x: mx, y: Math.floor(maxY / 2) };
    case "center":
      return { x: Math.floor(maxX / 2), y: Math.floor(maxY / 2) };
    case "center_right":
      return { x: Math.max(0, maxX - mx), y: Math.floor(maxY / 2) };
    case "bottom_left":
      return { x: mx, y: Math.max(0, maxY - my) };
    case "bottom_center":
      return { x: Math.floor(maxX / 2), y: Math.max(0, maxY - my) };
    default:
      return { x: Math.max(0, maxX - mx), y: Math.max(0, maxY - my) };
  }
}

export function computeTiedBackdropLayout(input: TiedBackdropLayoutInput): TiedBackdropLayout {
  const { width: wmW, height: wmH } = watermarkDimensions(input);
  const pad = Math.max(0, input.backdropPadding);
  const backdropW = wmW + pad * 4;
  const backdropH = wmH + pad * 4;
  const logoXOffset = clampBackdropLogoOffset(input.logoOffsetX, pad);
  const logoYOffset = clampBackdropLogoOffset(input.logoOffsetY, pad);

  let backdropX: number;
  let backdropY: number;

  if (input.positionMode === "manual") {
    const maxLogoX = Math.max(0, input.canvasWidth - wmW);
    const maxLogoY = Math.max(0, input.canvasHeight - wmH);
    const logoX = Math.min(Math.max(0, input.x), maxLogoX);
    const logoY = Math.min(Math.max(0, input.y), maxLogoY);
    backdropX = logoX - pad - logoXOffset;
    backdropY = logoY - pad - logoYOffset;
  } else {
    const pos = computePositionForBox(input, backdropW, backdropH);
    backdropX = pos.x;
    backdropY = pos.y;
  }

  const maxBackdropX = Math.max(0, input.canvasWidth - backdropW);
  const maxBackdropY = Math.max(0, input.canvasHeight - backdropH);
  backdropX = Math.min(Math.max(0, backdropX), maxBackdropX);
  backdropY = Math.min(Math.max(0, backdropY), maxBackdropY);

  const logoX = backdropX + pad + logoXOffset;
  const logoY = backdropY + pad + logoYOffset;

  return {
    backdropX,
    backdropY,
    backdropW,
    backdropH,
    logoX,
    logoY,
  };
}

export interface WatermarkPlacementResult {
  logo: { x: number; y: number; width: number; height: number };
  backdrop?: { x: number; y: number; width: number; height: number };
}

export function resolveWatermarkPlacement(
  layoutInput: WatermarkLayoutInput,
  backdrop: WatermarkBackdrop,
  backdropPadding: number,
  logoOffsetX: number,
  logoOffsetY: number
): WatermarkPlacementResult {
  const dims = watermarkDimensions(layoutInput);
  if (isTiedWatermarkBackdrop(backdrop)) {
    const layout = computeTiedBackdropLayout({
      ...layoutInput,
      backdropPadding,
      logoOffsetX,
      logoOffsetY,
    });
    return {
      logo: {
        x: layout.logoX,
        y: layout.logoY,
        width: dims.width,
        height: dims.height,
      },
      backdrop: {
        x: layout.backdropX,
        y: layout.backdropY,
        width: layout.backdropW,
        height: layout.backdropH,
      },
    };
  }
  const pos = computeWatermarkPosition(layoutInput);
  return { logo: pos };
}

export function parseBackdropColor(hex: string): { r: number; g: number; b: number } {
  const raw = hex.trim().replace(/^#/, "");
  if (raw.length >= 6) {
    const r = parseInt(raw.slice(0, 2), 16);
    const g = parseInt(raw.slice(2, 4), 16);
    const b = parseInt(raw.slice(4, 6), 16);
    if (!Number.isNaN(r) && !Number.isNaN(g) && !Number.isNaN(b)) {
      return { r, g, b };
    }
  }
  return { r: 0, g: 0, b: 0 };
}

export function backdropColorRgba(hex: string, opacity: number, factor = 1): string {
  const { r, g, b } = parseBackdropColor(hex);
  const a = Math.min(1, Math.max(0, (opacity / 100) * factor));
  return `rgba(${r},${g},${b},${a.toFixed(3)})`;
}

export interface BackdropRenderInput {
  backdrop: WatermarkBackdrop;
  backdropOpacity: number;
  backdropPadding: number;
  backdropColor: string;
  placement: { x: number; y: number; width: number; height: number };
  backdropBox?: { x: number; y: number; width: number; height: number };
  canvasWidth: number;
  canvasHeight: number;
  scale: number;
  preset: WatermarkPreset;
  positionMode: WatermarkPositionMode;
}

type CssLayer = Record<string, string | number | undefined>;

function backdropAlpha(opacity: number, factor = 1) {
  return Math.min(1, Math.max(0, (opacity / 100) * factor));
}

function watermarkBox(input: BackdropRenderInput) {
  const pad = input.backdropPadding;
  const box = input.backdropBox ?? {
    x: input.placement.x - pad,
    y: input.placement.y - pad,
    width: input.placement.width + pad * 2,
    height: input.placement.height + pad * 2,
  };
  return {
    left: box.x * input.scale,
    top: box.y * input.scale,
    width: box.width * input.scale,
    height: box.height * input.scale,
    radius: Math.max(8, Math.min(pad, 24)) * input.scale,
  };
}

function nearestCorner(input: BackdropRenderInput): "tl" | "tr" | "bl" | "br" {
  if (input.positionMode === "preset") {
    switch (input.preset) {
      case "top_left":
        return "tl";
      case "top_right":
        return "tr";
      case "bottom_left":
        return "bl";
      default:
        return "br";
    }
  }
  const cx = input.placement.x + input.placement.width / 2;
  const cy = input.placement.y + input.placement.height / 2;
  const left = cx < input.canvasWidth / 2;
  const top = cy < input.canvasHeight / 2;
  if (left && top) return "tl";
  if (!left && top) return "tr";
  if (left && !top) return "bl";
  return "br";
}

export function getWatermarkBackdropLayers(input: BackdropRenderInput): CssLayer[] {
  if (input.backdrop === "none") return [];

  const a = backdropAlpha(input.backdropOpacity);
  const fill = backdropColorRgba(input.backdropColor, input.backdropOpacity, 0.72);
  const shadow = backdropColorRgba(input.backdropColor, input.backdropOpacity, 0.45);
  const { r, g, b } = parseBackdropColor(input.backdropColor);
  const box = watermarkBox(input);
  const fullW = input.canvasWidth * input.scale;
  const fullH = input.canvasHeight * input.scale;

  switch (input.backdrop) {
    case "dark_rect":
      return [{ position: "absolute", left: box.left, top: box.top, width: box.width, height: box.height, background: fill }];
    case "dark_pill":
      return [{
        position: "absolute",
        left: box.left,
        top: box.top,
        width: box.width,
        height: box.height,
        background: fill,
        borderRadius: box.radius,
      }];
    case "shadow":
      return [
        {
          position: "absolute",
          left: box.left + 3 * input.scale,
          top: box.top + 4 * input.scale,
          width: box.width,
          height: box.height,
          background: shadow,
          borderRadius: box.radius,
        },
        {
          position: "absolute",
          left: box.left + 1 * input.scale,
          top: box.top + 2 * input.scale,
          width: box.width,
          height: box.height,
          background: shadow,
          borderRadius: box.radius,
        },
      ];
    case "dark_glow":
      return [
        {
          position: "absolute",
          left: box.left - 4 * input.scale,
          top: box.top - 4 * input.scale,
          width: box.width + 8 * input.scale,
          height: box.height + 8 * input.scale,
          background: `rgba(${r},${g},${b},${(0.22 * a).toFixed(3)})`,
          borderRadius: box.radius + 4 * input.scale,
          filter: `blur(${6 * input.scale}px)`,
        },
        {
          position: "absolute",
          left: box.left,
          top: box.top,
          width: box.width,
          height: box.height,
          background: `rgba(${r},${g},${b},${(0.55 * a).toFixed(3)})`,
          borderRadius: box.radius,
        },
      ];
    case "bottom_bar": {
      const barH = Math.max(48 * input.scale, fullH * 0.16);
      return [{ position: "absolute", left: 0, top: fullH - barH, width: fullW, height: barH, background: fill }];
    }
    case "top_bar": {
      const barH = Math.max(48 * input.scale, fullH * 0.16);
      return [{ position: "absolute", left: 0, top: 0, width: fullW, height: barH, background: fill }];
    }
    case "bottom_gradient":
      return [{
        position: "absolute",
        left: 0,
        top: fullH * 0.55,
        width: fullW,
        height: fullH * 0.45,
        background: `linear-gradient(to bottom, rgba(${r},${g},${b},0), rgba(${r},${g},${b},${(0.85 * a).toFixed(3)}))`,
      }];
    case "top_gradient":
      return [{
        position: "absolute",
        left: 0,
        top: 0,
        width: fullW,
        height: fullH * 0.45,
        background: `linear-gradient(to top, rgba(${r},${g},${b},0), rgba(${r},${g},${b},${(0.85 * a).toFixed(3)}))`,
      }];
    case "left_strip": {
      const stripW = Math.max(80 * input.scale, fullW * 0.22);
      return [{ position: "absolute", left: 0, top: 0, width: stripW, height: fullH, background: fill }];
    }
    case "right_strip": {
      const stripW = Math.max(80 * input.scale, fullW * 0.22);
      return [{ position: "absolute", left: fullW - stripW, top: 0, width: stripW, height: fullH, background: fill }];
    }
    case "vignette":
      return [{
        position: "absolute",
        inset: 0,
        background: `radial-gradient(ellipse at center, rgba(${r},${g},${b},0) 35%, rgba(${r},${g},${b},${(0.9 * a).toFixed(3)}) 100%)`,
      }];
    case "corner_fade": {
      const corner = nearestCorner(input);
      const at =
        corner === "tl"
          ? "left top"
          : corner === "tr"
            ? "right top"
            : corner === "bl"
              ? "left bottom"
              : "right bottom";
      return [{
        position: "absolute",
        inset: 0,
        background: `radial-gradient(ellipse 75% 75% at ${at}, rgba(${r},${g},${b},${(0.9 * a).toFixed(3)}) 0%, rgba(${r},${g},${b},0) 70%)`,
      }];
    }
    default:
      return [];
  }
}

export function getPreviewBackgroundStyle(bg: WatermarkPreviewBackground): Record<string, string> {
  switch (bg) {
    case "white":
      return { background: "#f8fafc" };
    case "light":
      return { background: "linear-gradient(135deg, #e2e8f0 0%, #cbd5e1 100%)" };
    case "photo":
      return {
        background:
          "linear-gradient(160deg, #64748b 0%, #94a3b8 35%, #e2e8f0 70%, #f8fafc 100%)",
      };
    default:
      return {
        backgroundColor: "#1a1a1a",
        backgroundImage:
          "linear-gradient(45deg,#2a2a2a 25%,transparent 25%),linear-gradient(-45deg,#2a2a2a 25%,transparent 25%),linear-gradient(45deg,transparent 75%,#2a2a2a 75%),linear-gradient(-45deg,transparent 75%,#2a2a2a 75%)",
        backgroundSize: "16px 16px",
        backgroundPosition: "0 0, 0 8px, 8px -8px, -8px 0px",
      };
  }
}

export const WATERMARK_PREVIEW_BACKGROUNDS: Array<{
  id: WatermarkPreviewBackground;
  label: string;
}> = [
  { id: "checker", label: "Тёмная" },
  { id: "white", label: "Белая" },
  { id: "light", label: "Светлая" },
  { id: "photo", label: "Фото" },
];
