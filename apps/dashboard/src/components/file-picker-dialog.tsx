"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, FileText, ImagePlus, Music, Search, Trash2, Upload, Video } from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
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
  deleteFile,
  type FileItem,
  type FileReference,
  getFileReferences,
  getFiles,
  uploadFile,
} from "@/lib/api";

interface FilePickerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (file: FileItem) => void;
  siteId: string;
  accept?: string;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function FilePickerDialog({
  open,
  onOpenChange,
  onSelect,
  siteId,
  accept,
}: FilePickerDialogProps) {
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);
  const [tab, setTab] = useState("library");
  const [dragOver, setDragOver] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<{
    file: FileItem;
    refs: FileReference[];
  } | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const queryClient = useQueryClient();

  const acceptTypes = accept ? accept.split(",").map((s) => s.trim()) : [];
  const fileType = acceptTypes.length > 0
    ? acceptTypes.some((m) => m.includes("image/"))
      ? "image"
      : acceptTypes.some((m) => m.includes("video/"))
        ? "video"
        : acceptTypes.some((m) => m.includes("audio/"))
          ? "audio"
          : acceptTypes.some(
              (m) =>
                m.startsWith("application/pdf") ||
                m.startsWith("application/msword") ||
                m.startsWith("application/vnd.") ||
                m.startsWith("text/"),
            )
            ? "document"
            : acceptTypes.some(
                  (m) =>
                    m.startsWith("application/zip") ||
                    m.startsWith("application/gzip") ||
                    m.startsWith("application/x-tar") ||
                    m.startsWith("application/x-7z") ||
                    m.startsWith("application/x-rar"),
                )
              ? "archive"
              : undefined
    : undefined;

  const { data, isLoading } = useQuery({
    queryKey: ["files", siteId, page, search, fileType],
    queryFn: () =>
      getFiles(siteId, {
        page,
        search: search || undefined,
        type: fileType,
      }),
    enabled: open,
  });

  const filteredItems = (() => {
    const items = data?.items ?? [];
    if (!accept || accept.length === 0) return items;
    const patterns = accept.split(",").map((s) => s.trim());
    return items.filter((file) => {
      return patterns.some((pattern) => {
        if (pattern.endsWith("/*")) {
          const prefix = pattern.slice(0, -2);
          return file.mime_type.startsWith(prefix);
        }
        return file.mime_type === pattern;
      });
    });
  })();

  const uploadMutation = useMutation({
    mutationFn: ({
      file,
      provider,
    }: {
      file: File;
      provider: "filesystem" | "s3";
    }) => uploadFile(siteId, file, provider),
    onSuccess: (file) => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      toast.success("File uploaded");
      onSelect(file);
      onOpenChange(false);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (fileId: string) => deleteFile(siteId, fileId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      toast.success("File deleted");
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

  const handleDelete = async (file: FileItem) => {
    const refs = await getFileReferences(siteId, file.id).catch(() => []);
    if (refs.length > 0) {
      setPendingDelete({ file, refs });
    } else {
      deleteMutation.mutate(file.id);
    }
  };

  const confirmDelete = () => {
    if (pendingDelete) {
      deleteMutation.mutate(pendingDelete.file.id);
      setPendingDelete(null);
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="flex max-h-[80vh] flex-col sm:max-w-3xl">
          <DialogHeader>
            <DialogTitle>File Library</DialogTitle>
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
                ) : !filteredItems.length ? (
                  <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
                    No files found.
                  </div>
                ) : (
                  <div className="grid grid-cols-3 gap-3 overflow-y-auto sm:grid-cols-4">
                    {filteredItems.map((file) => (
                      <FileGridItem
                        key={file.id}
                        file={file}
                        onSelect={() => {
                          onSelect(file);
                          onOpenChange(false);
                        }}
                        onDelete={() => handleDelete(file)}
                      />
                    ))}
                  </div>
                )}

                {data && data.total > data.per_page && (
                  <div className="flex items-center justify-between text-sm text-muted-foreground">
                    <span>
                      {filteredItems.length} of {data.total} files
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
                  {fileType === "image" ? (
                    <ImagePlus data-icon="inline-start" />
                  ) : fileType === "video" ? (
                    <Video data-icon="inline-start" />
                  ) : fileType === "audio" ? (
                    <Music data-icon="inline-start" />
                  ) : fileType === "document" ? (
                    <FileText data-icon="inline-start" />
                  ) : fileType === "archive" ? (
                    <Archive data-icon="inline-start" />
                  ) : (
                    <Upload data-icon="inline-start" />
                  )}
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

      <AlertDialog
        open={!!pendingDelete}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete file?</AlertDialogTitle>
            <AlertDialogDescription>
              This file is used in {pendingDelete?.refs.length} content item(s)
              (
              {pendingDelete &&
                [
                  ...new Set(pendingDelete.refs.map((r) => r.collection_name)),
                ].join(", ")}
              ).
              <br />
              Deleting it may break those pages.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteMutation.isPending}
            >
              {deleteMutation.isPending ? "Deleting..." : "Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function FileGridItem({
  file,
  onSelect,
  onDelete,
}: {
  file: FileItem;
  onSelect: () => void;
  onDelete: () => void;
}) {
  const isImage = file.mime_type.startsWith("image/");
  const isVideo = file.mime_type.startsWith("video/");

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
          src={file.thumbnail_url || file.url}
          alt={file.original_name}
          className="size-full object-cover"
        />
      ) : isVideo && file.thumbnail_url ? (
        <img
          src={file.thumbnail_url}
          alt={file.original_name}
          className="size-full object-cover"
        />
      ) : (
        <div className="flex size-full flex-col items-center justify-center bg-muted p-2">
          <Badge variant="secondary" className="text-xs">
            {file.mime_type.split("/")[1]?.toUpperCase() || "FILE"}
          </Badge>
          <p className="mt-1 truncate text-center text-xs text-muted-foreground">
            {file.original_name}
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
        <p className="truncate text-xs text-white">{file.original_name}</p>
        <p className="text-[10px] text-white/70">{formatFileSize(file.size)}</p>
      </div>
    </button>
  );
}
