"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ImagePlus, Search, Trash2, Upload } from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  deleteMedia,
  getMedia,
  getMediaReferences,
  type Media,
  uploadMedia,
} from "@/lib/api";

interface MediaPickerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (media: Media) => void;
  siteId: string;
  accept?: string;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function MediaPickerDialog({
  open,
  onOpenChange,
  onSelect,
  siteId,
  accept,
}: MediaPickerDialogProps) {
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);
  const [tab, setTab] = useState("library");
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ["media", siteId, page, search],
    queryFn: () => getMedia(siteId, { page, search: search || undefined }),
    enabled: open,
  });

  const uploadMutation = useMutation({
    mutationFn: ({
      file,
      provider,
    }: {
      file: File;
      provider: "filesystem" | "s3";
    }) => uploadMedia(siteId, file, provider),
    onSuccess: (media) => {
      queryClient.invalidateQueries({ queryKey: ["media", siteId] });
      toast.success("File uploaded");
      onSelect(media);
      onOpenChange(false);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (mediaId: string) => deleteMedia(siteId, mediaId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["media", siteId] });
      toast.success("Media deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const handleFileSelect = (files: FileList | null) => {
    if (!files || files.length === 0) return;
    const file = files[0];
    uploadMutation.mutate({ file, provider: "filesystem" });
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    handleFileSelect(e.dataTransfer.files);
  };

  const handleDelete = async (media: Media) => {
    const refs = await getMediaReferences(siteId, media.id).catch(() => []);
    if (refs.length > 0) {
      const names = refs.map((r) => r.collection_name).join(", ");
      if (
        !window.confirm(
          `This media is used in ${refs.length} content item(s) (${names}). Delete anyway?`,
        )
      ) {
        return;
      }
    }
    deleteMutation.mutate(media.id);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[80vh] flex-col sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Media Library</DialogTitle>
          <DialogDescription>
            Select an existing file or upload a new one.
          </DialogDescription>
        </DialogHeader>

        <Tabs
          value={tab}
          onValueChange={setTab}
          className="flex flex-1 flex-col overflow-hidden"
        >
          <TabsList>
            <TabsTrigger value="library">Library</TabsTrigger>
            <TabsTrigger value="upload">Upload</TabsTrigger>
          </TabsList>

          <TabsContent value="library" className="flex-1 overflow-hidden">
            <div className="flex flex-col gap-3 overflow-hidden">
              <div className="relative">
                <Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
                <Input
                  placeholder="Search files..."
                  value={search}
                  onChange={(e) => {
                    setSearch(e.target.value);
                    setPage(1);
                  }}
                  className="pl-8"
                />
              </div>

              {isLoading ? (
                <div className="grid grid-cols-3 gap-3 sm:grid-cols-4">
                  {["a", "b", "c", "d", "e", "f", "g", "h"].map((id) => (
                    <Skeleton key={id} className="aspect-square rounded-lg" />
                  ))}
                </div>
              ) : !data?.items.length ? (
                <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
                  No media files found.
                </div>
              ) : (
                <div className="grid grid-cols-3 gap-3 overflow-y-auto sm:grid-cols-4">
                  {data.items.map((media) => (
                    <MediaGridItem
                      key={media.id}
                      media={media}
                      onSelect={() => {
                        onSelect(media);
                        onOpenChange(false);
                      }}
                      onDelete={() => handleDelete(media)}
                    />
                  ))}
                </div>
              )}

              {data && data.total > data.per_page && (
                <div className="flex items-center justify-between text-sm text-muted-foreground">
                  <span>
                    {data.items.length} of {data.total} files
                  </span>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={page <= 1}
                      onClick={() => setPage((p) => p - 1)}
                    >
                      Previous
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={data.items.length < data.per_page}
                      onClick={() => setPage((p) => p + 1)}
                    >
                      Next
                    </Button>
                  </div>
                </div>
              )}
            </div>
          </TabsContent>

          <TabsContent value="upload" className="flex-1">
            {/* biome-ignore lint/a11y/noStaticElementInteractions: drop zone needs drag events */}
            <div
              className={`flex flex-col items-center justify-center gap-4 rounded-lg border-2 border-dashed p-12 transition-colors ${
                dragOver
                  ? "border-primary bg-primary/5"
                  : "border-muted-foreground/25"
              }`}
              onDragOver={(e) => {
                e.preventDefault();
                setDragOver(true);
              }}
              onDragLeave={() => setDragOver(false)}
              onDrop={handleDrop}
            >
              <Upload className="size-10 text-muted-foreground" />
              <div className="text-center">
                <p className="font-medium">
                  Drag and drop a file here, or click to browse
                </p>
                <p className="text-sm text-muted-foreground">
                  {accept || "Images, videos, PDFs, and more"}
                </p>
              </div>
              <Button
                variant="outline"
                onClick={() => fileInputRef.current?.click()}
                disabled={uploadMutation.isPending}
              >
                <ImagePlus data-icon="inline-start" />
                {uploadMutation.isPending ? "Uploading..." : "Choose File"}
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                className="hidden"
                accept={accept}
                onChange={(e) => handleFileSelect(e.target.files)}
              />
            </div>
          </TabsContent>
        </Tabs>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function MediaGridItem({
  media,
  onSelect,
  onDelete,
}: {
  media: Media;
  onSelect: () => void;
  onDelete: () => void;
}) {
  const isImage = media.mime_type.startsWith("image/");

  return (
    <button
      type="button"
      className="group relative aspect-square cursor-pointer overflow-hidden rounded-lg border text-left transition-shadow hover:shadow-md"
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
    >
      {isImage ? (
        <img
          src={media.thumbnail_url || media.url}
          alt={media.original_name}
          className="size-full object-cover"
        />
      ) : (
        <div className="flex size-full flex-col items-center justify-center bg-muted p-2">
          <Badge variant="secondary" className="text-xs">
            {media.mime_type.split("/")[1]?.toUpperCase() || "FILE"}
          </Badge>
          <p className="mt-1 truncate text-center text-xs text-muted-foreground">
            {media.original_name}
          </p>
        </div>
      )}
      <div className="absolute right-1 top-1 hidden group-hover:block">
        <Button
          variant="ghost"
          size="icon-sm"
          className="bg-black/40 text-white hover:bg-black/60 hover:text-white"
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
        >
          <Trash2 />
        </Button>
      </div>
      <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/60 to-transparent p-1.5">
        <p className="truncate text-xs text-white">{media.original_name}</p>
        <p className="text-[10px] text-white/70">
          {formatFileSize(media.size)}
        </p>
      </div>
    </button>
  );
}
