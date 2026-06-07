import { convertFileSrc } from "@tauri-apps/api/core";
import { readLocalImageDataUrl, resolveLocalImagePath } from "@/lib/tauri";

const localSrcCache = new Map<string, string>();

export function isLocalImageRef(url: string | null | undefined): boolean {
  return Boolean(url?.startsWith("local:"));
}

export async function resolvePostImageSrc(
  url: string | null | undefined
): Promise<string | undefined> {
  if (!url) return undefined;
  if (!isLocalImageRef(url)) return url;

  const cached = localSrcCache.get(url);
  if (cached) return cached;

  try {
    const absolutePath = await resolveLocalImagePath(url);
    const src = convertFileSrc(absolutePath);
    localSrcCache.set(url, src);
    return src;
  } catch {
    const dataUrl = await readLocalImageDataUrl(url);
    localSrcCache.set(url, dataUrl);
    return dataUrl;
  }
}

export async function resolvePostImageSrcFallback(
  url: string
): Promise<string | undefined> {
  if (!isLocalImageRef(url)) return undefined;
  try {
    return await readLocalImageDataUrl(url);
  } catch {
    return undefined;
  }
}
