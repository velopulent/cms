import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  FileText,
  ImagePlus,
  Music,
  Search,
  Trash2,
  Upload,
  Video,
} from "lucide-react";
import { useCallback, useMemo, useRef, useState } from "react";
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

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type StorageProvider = "filesystem" | "s3";

type FileType = "image" | "video" | "audio" | "document" | "archive";

interface FilePickerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (file: FileItem) => void;
  siteId: string;
  /** Comma-separated MIME types / wildcards, e.g. "image/*,application/pdf" */
  accept?: string;
  /** Storage provider to use when uploading. Defaults to "filesystem". */
  uploadProvider?: StorageProvider;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Derives a broad file-type category from a comma-separated MIME accept string.
 * Returns `undefined` when the accept prop is absent or doesn't map to a known
 * category, so the API query is made without a type filter.
 */
function deriveFileType(accept: string | undefined): FileType | undefined {
  if (!accept) return undefined;

  const types = accept.split(",").map((s) => s.trim());

  if (types.some((m) => m.startsWith("image/"))) return "image";
  if (types.some((m) => m.startsWith("video/"))) return "video";
  if (types.some((m) => m.startsWith("audio/"))) return "audio";
  if (
    types.some(
      (m) =>
        m.startsWith("application/pdf") ||
        m.startsWith("application/msword") ||
        m.startsWith("application/vnd.") ||
        m.startsWith("text/"),
    )
  )
    return "document";
  if (
    types.some(
      (m) =>
        m.startsWith("application/zip") ||
        m.startsWith("application/gzip") ||
        m.startsWith("application/x-tar") ||
        m.startsWith("application/x-7z") ||
        m.startsWith("application/x-rar"),
    )
  )
    return "archive";

  return undefined;
}

/** Returns true when a file's MIME type matches one of the accept patterns. */
function matchesAccept(mimeType: string, accept: string): boolean {
  return accept
    .split(",")
    .map((s) => s.trim())
    .some((pattern) => {
      if (pattern.endsWith("/*")) {
        return mimeType.startsWith(pattern.slice(0, -1)); // e.g. "image/" prefix check
      }
      return mimeType === pattern;
    });
}

// ---------------------------------------------------------------------------
// FilePickerDialog
// ---------------------------------------------------------------------------

export function FilePickerDialog({
  open,
  onOpenChange,
  onSelect,
  siteId,
  accept,
  uploadProvider = "filesystem",
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

  // Derived values – memoised so they don't recalculate on every render.
  const fileType = useMemo(() => deriveFileType(accept), [accept]);

  const { data, isLoading, isError } = useQuery({
    queryKey: ["files", siteId, page, search, fileType],
    queryFn: () =>
      getFiles(siteId, {
        page,
        search: search || undefined,
        type: fileType,
      }),
    enabled: open,
  });

  // Client-side filtering on top of the server-side type filter, to handle
  // cases where the accept prop contains multiple specific MIME types.
  const filteredItems = useMemo(() => {
    const items = data?.items ?? [];
    if (!accept) return items;
    return items.filter((file) => matchesAccept(file.mime_type, accept));
  }, [data?.items, accept]);

  // Pagination: the last page is reached when the server returns fewer items
  // than requested, OR when (page * per_page) >= total.
  const isLastPage = useMemo(() => {
    if (!data) return true;
    return (
      data.items.length < data.per_page || page * data.per_page >= data.total
    );
  }, [data, page]);

  // ---------------------------------------------------------------------------
  // Mutations
  // ---------------------------------------------------------------------------

  const uploadMutation = useMutation({
    mutationFn: ({ file }: { file: File }) =>
      uploadFile(siteId, file, uploadProvider),
    onSuccess: (uploaded) => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      toast.success("File uploaded");
      onSelect(uploaded);
      onOpenChange(false);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (fileId: string) => deleteFile(siteId, fileId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      toast.success("File deleted");
      setPendingDelete(null);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  const handleFileSelect = useCallback(
    (files: FileList | null) => {
      const file = files?.[0];
      if (!file) return;
      uploadMutation.mutate({ file });
    },
    [uploadMutation],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLLabelElement>) => {
      e.preventDefault();
      setDragOver(false);
      handleFileSelect(e.dataTransfer.files);
    },
    [handleFileSelect],
  );

  const handleDelete = useCallback(
    async (file: FileItem) => {
      let refs: FileReference[] = [];
      try {
        refs = await getFileReferences(siteId, file.id);
      } catch {
        // Treat a failed reference lookup the same as "no references" so the
        // user can still delete the file, but warn them just in case.
        toast.warning(
          "Could not check file references. Proceeding with delete.",
        );
      }

      if (refs.length > 0) {
        setPendingDelete({ file, refs });
      } else {
        deleteMutation.mutate(file.id);
      }
    },
    [siteId, deleteMutation],
  );

  const confirmDelete = useCallback(() => {
    if (!pendingDelete) return;
    deleteMutation.mutate(pendingDelete.file.id);
  }, [pendingDelete, deleteMutation]);

  const handleSearchChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setSearch(e.target.value);
      setPage(1);
    },
    [],
  );

  // Reset local state each time the dialog is closed.
  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (!next) {
        setSearch("");
        setPage(1);
        setTab("library");
      }
      onOpenChange(next);
    },
    [onOpenChange],
  );

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  const uploadIcon =
    fileType === "image" ? (
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
    );

  const pendingCollections = pendingDelete
    ? [...new Set(pendingDelete.refs.map((r) => r.collection_name))].join(", ")
    : "";

  return (
    <>
      <Dialog open={open} onOpenChange={handleOpenChange}>
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

            {/* ----------------------------------------------------------------
                Library tab
            ---------------------------------------------------------------- */}
            <TabsContent
              value="library"
              className="flex flex-1 flex-col gap-3 overflow-hidden"
            >
              <div className="relative">
                <Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
                <Input
                  placeholder="Search files…"
                  value={search}
                  onChange={handleSearchChange}
                  className="pl-8"
                />
              </div>

              {isLoading ? (
                <div className="grid grid-cols-3 gap-3 sm:grid-cols-4">
                  {Array.from({ length: 8 }, (_, i) => (
                    <Skeleton key={i} className="aspect-square rounded-lg" />
                  ))}
                </div>
              ) : isError ? (
                <div className="flex flex-1 items-center justify-center text-destructive text-sm">
                  Failed to load files. Please try again.
                </div>
              ) : filteredItems.length === 0 ? (
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
                        handleOpenChange(false);
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
                      disabled={isLastPage}
                      onClick={() => setPage((p) => p + 1)}
                    >
                      Next
                    </Button>
                  </div>
                </div>
              )}
            </TabsContent>

            {/* ----------------------------------------------------------------
                Upload tab — using a <label> as the drop zone so the hidden
                <input> receives clicks without needing an imperative ref call,
                and biome's a11y/noStaticElementInteractions lint is satisfied.
            ---------------------------------------------------------------- */}
            <TabsContent value="upload" className="flex-1">
              <label
                htmlFor="file-picker-input"
                className={[
                  "flex flex-col items-center justify-center gap-4 rounded-lg border-2 border-dashed p-12 transition-colors cursor-pointer",
                  dragOver
                    ? "border-primary bg-primary/5"
                    : "border-muted-foreground/25 hover:border-muted-foreground/50",
                ].join(" ")}
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
                    {accept ?? "Images, videos, PDFs, and more"}
                  </p>
                </div>
                <Button
                  variant="outline"
                  // Prevent the label's click from firing twice (once from the
                  // button, once bubbling up to the label → input).
                  onClick={(e) => e.preventDefault()}
                  disabled={uploadMutation.isPending}
                >
                  {uploadIcon}
                  {uploadMutation.isPending ? "Uploading…" : "Choose File"}
                </Button>
                <input
                  id="file-picker-input"
                  ref={fileInputRef}
                  type="file"
                  className="sr-only"
                  accept={accept}
                  onChange={(e) => handleFileSelect(e.target.files)}
                  // Reset value so the same file can be re-uploaded if needed.
                  onClick={(e) => {
                    (e.target as HTMLInputElement).value = "";
                  }}
                />
              </label>
            </TabsContent>
          </Tabs>

          <DialogFooter>
            <Button variant="outline" onClick={() => handleOpenChange(false)}>
              Cancel
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* -----------------------------------------------------------------------
          Delete-with-references confirmation dialog
      ----------------------------------------------------------------------- */}
      <AlertDialog
        open={!!pendingDelete}
        onOpenChange={(isOpen) => {
          if (!isOpen) setPendingDelete(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete file?</AlertDialogTitle>
            <AlertDialogDescription>
              This file is used in <strong>{pendingDelete?.refs.length}</strong>{" "}
              content item
              {pendingDelete?.refs.length === 1 ? "" : "s"}
              {pendingCollections ? ` (${pendingCollections})` : ""}. Deleting
              it may break those pages.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteMutation.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteMutation.isPending}
            >
              {deleteMutation.isPending ? "Deleting…" : "Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

// ---------------------------------------------------------------------------
// FileGridItem
// ---------------------------------------------------------------------------

interface FileGridItemProps {
  file: FileItem;
  onSelect: () => void;
  onDelete: () => void;
}

function FileGridItem({ file, onSelect, onDelete }: FileGridItemProps) {
  const isImage = file.mime_type.startsWith("image/");
  const isVideo = file.mime_type.startsWith("video/");
  const hasPreview = isImage || (isVideo && !!file.thumbnail_url);

  return (
    <button
      type="button"
      className="group relative aspect-square cursor-pointer overflow-hidden rounded-lg border text-left transition-shadow hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
      onClick={onSelect}
    >
      {hasPreview ? (
        <img
          src={file.thumbnail_url ?? file.url}
          alt={file.original_name}
          className="size-full object-cover"
          loading="lazy"
        />
      ) : (
        <div className="flex size-full flex-col items-center justify-center gap-1 bg-muted p-2">
          <Badge variant="secondary" className="text-xs">
            {file.mime_type.split("/")[1]?.toUpperCase() ?? "FILE"}
          </Badge>
          <p className="w-full truncate text-center text-xs text-muted-foreground">
            {file.original_name}
          </p>
        </div>
      )}

      {/* Delete button — only visible on hover/focus-within */}
      <div className="absolute right-1 top-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={`Delete ${file.original_name}`}
          className="bg-black/40 text-white hover:bg-black/60 hover:text-white"
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
        >
          <Trash2 />
        </Button>
      </div>

      {/* Filename / size overlay */}
      <div className="absolute bottom-0 left-0 right-0 bg-linear-to-t from-black/60 to-transparent p-1.5">
        <p className="truncate text-xs text-white">{file.original_name}</p>
        <p className="text-[10px] text-white/70">{formatFileSize(file.size)}</p>
      </div>
    </button>
  );
}
