import Image from "@tiptap/extension-image";
import { ReactNodeViewRenderer } from "@tiptap/react";
import { TiptapImageComponent } from "@/components/tiptap-image-component";

declare module "@tiptap/extension-image" {
  interface ImageOptions {
    siteId?: string;
  }
}

export const ImageWithControls = Image.extend({
  addOptions() {
    return {
      inline: false,
      allowBase64: false,
      HTMLAttributes: {},
      resize: false as const,
      siteId: undefined as string | undefined,
    };
  },

  addNodeView() {
    return ReactNodeViewRenderer(TiptapImageComponent);
  },
}).configure({
  HTMLAttributes: { class: "rounded-lg max-w-full h-auto" },
});
