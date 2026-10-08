import { ImageZoom } from "fumadocs-ui/components/image-zoom";

type AdaptiveScreenshotProps = {
  light: string;
  dark: string;
  alt: string;
  caption?: string;
  width?: number;
  height?: number;
};

/** Displays the dashboard capture that matches the active documentation theme. */
export function AdaptiveScreenshot({
  light,
  dark,
  alt,
  caption,
  width = 1920,
  height = 1080,
}: AdaptiveScreenshotProps) {
  return (
    <figure className="my-6 flex flex-col gap-2">
      <div className="overflow-hidden rounded-xl border bg-muted shadow-sm">
        <ImageZoom
          src={light}
          alt={alt}
          width={width}
          height={height}
          className="block h-auto w-full dark:hidden"
        />
        <ImageZoom
          src={dark}
          alt={alt}
          width={width}
          height={height}
          className="hidden h-auto w-full dark:block"
        />
      </div>
      {caption ? (
        <figcaption className="text-center text-sm text-muted-foreground">
          {caption}
        </figcaption>
      ) : null}
    </figure>
  );
}
