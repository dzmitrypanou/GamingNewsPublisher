import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface PostPreviewProps {
  platform: "vk" | "telegram" | "unified";
  title: string;
  text: string;
  hashtags: string;
  imageUrl: string | null;
}

export function PostPreview({ platform, title, text, hashtags, imageUrl }: PostPreviewProps) {
  const caption = [title, "", text, "", hashtags].filter(Boolean).join("\n");
  const vkLimit = 4096;
  const tgLimit = 1024;
  const charLimit = platform === "telegram" ? tgLimit : platform === "vk" ? vkLimit : tgLimit;
  const isOverVk = caption.length > vkLimit;
  const isOverTg = caption.length > tgLimit;
  const isOverLimit = platform === "unified" ? isOverTg || isOverVk : caption.length > charLimit;

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex flex-wrap items-center gap-2 text-base">
          {platform === "unified" ? (
            <>
              <span>Превью</span>
              <span className="text-xs font-normal text-muted-foreground">
                <span className="text-[#0077FF]">VK</span> {caption.length}/{vkLimit}
                <span className="mx-1.5">·</span>
                <span className="text-[#2AABEE]">TG</span> {caption.length}/{tgLimit}
              </span>
            </>
          ) : platform === "vk" ? (
            <>
              <span className="text-[#0077FF]">VK</span>
              <span className="text-xs font-normal text-muted-foreground">
                {caption.length}/{vkLimit}
              </span>
            </>
          ) : (
            <>
              <span className="text-[#2AABEE]">Telegram</span>
              <span className="text-xs font-normal text-muted-foreground">
                {caption.length}/{tgLimit}
              </span>
            </>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="rounded-lg border border-border bg-background p-4">
          {imageUrl && (
            <div className="mb-3 overflow-hidden rounded-md bg-secondary">
              <img
                src={imageUrl}
                alt="Preview"
                className="max-h-48 w-full object-cover"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = "none";
                }}
              />
            </div>
          )}
          <p className="text-sm font-bold">{title || "Заголовок"}</p>
          <p className="mt-2 whitespace-pre-wrap text-sm text-secondary-foreground">
            {text || "Текст поста"}
          </p>
          {hashtags && (
            <p className="mt-2 text-sm text-primary">{hashtags}</p>
          )}
        </div>
        {isOverLimit && (
          <p className="mt-2 text-xs text-warning">
            {platform === "unified"
              ? `Текст превышает лимит${isOverTg ? " Telegram" : ""}${isOverTg && isOverVk ? " и" : ""}${isOverVk ? " VK" : ""} — будет обрезан при публикации`
              : `Текст превышает лимит ${platform === "telegram" ? "Telegram" : "VK"} — будет обрезан при публикации`}
          </p>
        )}
      </CardContent>
    </Card>
  );
}
