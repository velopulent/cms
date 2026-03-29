import type { NodeViewProps } from "@tiptap/react";
import { NodeViewWrapper } from "@tiptap/react";
import { Pencil, Trash2 } from "lucide-react";
import { useState } from "react";
import { MediaPickerDialog } from "@/components/media-picker-dialog";
import type { Media } from "@/lib/api";

export function TiptapImageComponent(props: NodeViewProps) {
  const { node, updateAttributes, deleteNode, editor, selected } = props;
  const [pickerOpen, setPickerOpen] = useState(false);
  const siteId = editor.extensionManager.extensions.find(
    (ext) => ext.name === "image",
  )?.options.siteId as string | undefined;

  const src = node.attrs.src as string;

  const handleReplace = (media: Media) => {
    updateAttributes({ src: media.url });
    setPickerOpen(false);
  };

  return (
    <NodeViewWrapper
      as="div"
      className={`node-image group relative my-4 inline-block max-w-[400px] cursor-default ${selected ? "ring-2 ring-primary ring-offset-2 rounded-lg" : ""}`}
      data-drag-handle
    >
      <div className="pointer-events-none absolute inset-0 z-10 flex items-start justify-end gap-1 rounded-lg bg-black/0 p-1.5 opacity-0 transition-all group-hover:bg-black/30 group-hover:opacity-100">
        {siteId && (
          <button
            type="button"
            className="pointer-events-auto cursor-pointer rounded-md bg-black/50 p-1.5 text-white transition-colors hover:bg-black/70"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setPickerOpen(true);
            }}
            title="Replace image"
          >
            <Pencil className="size-3.5" />
          </button>
        )}
        <button
          type="button"
          className="pointer-events-auto cursor-pointer rounded-md bg-black/50 p-1.5 text-white transition-colors hover:bg-destructive"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            deleteNode();
          }}
          title="Delete image"
        >
          <Trash2 className="size-3.5" />
        </button>
      </div>

      <img
        src={src}
        alt={node.attrs.alt || ""}
        title={node.attrs.title || ""}
        className="pointer-events-auto block max-w-full rounded-lg"
      />

      {siteId && (
        <MediaPickerDialog
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          onSelect={handleReplace}
          siteId={siteId}
          accept="image/*"
        />
      )}
    </NodeViewWrapper>
  );
}
