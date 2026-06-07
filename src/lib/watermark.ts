import type { WatermarkPositionMode, WatermarkPreset, WatermarkSizeMode } from "./types";

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
