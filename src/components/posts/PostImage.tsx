import { useEffect, useState } from "react";
import { isLocalImageRef, resolvePostImageSrc, resolvePostImageSrcFallback } from "@/lib/image";

type PostImageProps = {
  url: string | null | undefined;
  alt?: string;
  className?: string;
  onError?: (e: React.SyntheticEvent<HTMLImageElement>) => void;
};

export function PostImage({ url, alt = "", className, onError }: PostImageProps) {
  const [src, setSrc] = useState<string | undefined>();
  const [triedFallback, setTriedFallback] = useState(false);

  useEffect(() => {
    let active = true;
    setTriedFallback(false);
    resolvePostImageSrc(url)
      .then((resolved) => {
        if (active) setSrc(resolved);
      })
      .catch(() => {
        if (active) setSrc(undefined);
      });
    return () => {
      active = false;
    };
  }, [url]);

  const handleError = (e: React.SyntheticEvent<HTMLImageElement>) => {
    if (url && isLocalImageRef(url) && !triedFallback) {
      setTriedFallback(true);
      resolvePostImageSrcFallback(url)
        .then((fallback) => {
          if (fallback) {
            setSrc(fallback);
            return;
          }
          onError?.(e);
        })
        .catch(() => onError?.(e));
      return;
    }
    onError?.(e);
  };

  if (!src) return null;

  return (
    <img
      src={src}
      alt={alt}
      className={className}
      onError={handleError}
    />
  );
}
