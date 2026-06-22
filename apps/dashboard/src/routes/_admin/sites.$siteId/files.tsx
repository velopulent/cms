import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  Check,
  Download,
  ImagePlus,
  RotateCcw,
  Search,
  SquareArrowOutUpRight,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
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
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { VideoPlayer } from "@/components/video-player";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import {
  batchDeleteFiles,
  batchPermanentDeleteFiles,
  batchRestoreFiles,
  deleteFile,
  type FileItem,
  type FileReference,
  getFileReferences,
  getFiles,
  restoreFile,
  uploadFile,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/files")({
  component: FilesPage,
});

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function formatDeletedDate(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Deleted today";
  if (diffDays === 1) return "Deleted yesterday";
  if (diffDays < 30) return `Deleted ${diffDays} days ago`;
  return `Deleted ${formatDate(dateStr)}`;
}

function FilesPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search, 300);
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [page, setPage] = useState(1);
  const [view, setView] = useState<"all" | "trash">("all");
  const [selectedFile, setSelectedFile] = useState<FileItem | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<{
    file: FileItem;
    refs: FileReference[];
  } | null>(null);
  const [batchDeleteConfirm, setBatchDeleteConfirm] = useState(false);
  const [batchPermanentDeleteConfirm, setBatchPermanentDeleteConfirm] =
    useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const isTrash = view === "trash";

  const { data, isLoading } = useQuery({
    queryKey: ["files", siteId, page, debouncedSearch, typeFilter, view],
    queryFn: () =>
      getFiles(siteId, {
        page,
        search: debouncedSearch || undefined,
        type: typeFilter === "all" ? undefined : typeFilter,
        trashed: isTrash,
      }),
  });

  // Reset to the first page when the debounced search term changes.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset only on term change
  useEffect(() => {
    setPage(1);
  }, [debouncedSearch]);

  const uploadMutation = useMutation({
    mutationFn: (file: File) => uploadFile(siteId, file, "filesystem"),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      toast.success("File uploaded successfully");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (fileId: string) => deleteFile(siteId, fileId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      setDetailsOpen(false);
      setSelectedFile(null);
      toast.success("File moved to trash");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const restoreMutation = useMutation({
    mutationFn: (fileId: string) => restoreFile(siteId, fileId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      setDetailsOpen(false);
      setSelectedFile(null);
      toast.success("File restored");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const batchDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => batchDeleteFiles(siteId, ids),
    onSuccess: (_data, ids) => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      setSelectedIds(new Set());
      toast.success(`${ids.length} file(s) moved to trash`);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const batchRestoreMutation = useMutation({
    mutationFn: (ids: string[]) => batchRestoreFiles(siteId, ids),
    onSuccess: (_data, ids) => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      setSelectedIds(new Set());
      toast.success(`${ids.length} file(s) restored`);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const batchPermanentDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => batchPermanentDeleteFiles(siteId, ids),
    onSuccess: (_data, ids) => {
      queryClient.invalidateQueries({ queryKey: ["files", siteId] });
      setSelectedIds(new Set());
      toast.success(`${ids.length} file(s) permanently deleted`);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const handleFileSelect = (files: FileList | null) => {
    if (!files) return;
    for (const file of Array.from(files)) {
      uploadMutation.mutate(file);
    }
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

  const toggleSelect = (fileId: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(fileId)) {
        next.delete(fileId);
      } else {
        next.add(fileId);
      }
      return next;
    });
  };

  const handleViewChange = (v: string) => {
    setView(v as "all" | "trash");
    setSelectedIds(new Set());
    setPage(1);
  };

  const hasSelection = selectedIds.size > 0;

  const handleBatchDelete = () => {
    batchDeleteMutation.mutate(Array.from(selectedIds));
    setBatchDeleteConfirm(false);
  };

  const handleBatchRestore = () => {
    batchRestoreMutation.mutate(Array.from(selectedIds));
  };

  const handleBatchPermanentDelete = () => {
    batchPermanentDeleteMutation.mutate(Array.from(selectedIds));
    setBatchPermanentDeleteConfirm(false);
  };

  return (
    <div className="flex flex-col gap-6 p-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Files</h1>
          <p className="text-sm text-muted-foreground">
            {isTrash
              ? "Deleted files are kept here until permanently removed"
              : "Upload and manage files for your site"}
          </p>
        </div>
        {!isTrash && (
          // biome-ignore lint/a11y/noStaticElementInteractions: drop zone
          <div
            className="flex items-center gap-2"
            onDragOver={(e) => {
              e.preventDefault();
              setDragOver(true);
            }}
            onDragLeave={() => setDragOver(false)}
            onDrop={handleDrop}
          >
            <Button
              variant="outline"
              onClick={() => fileInputRef.current?.click()}
              disabled={uploadMutation.isPending}
            >
              <Upload data-icon="inline-start" />
              {uploadMutation.isPending ? "Uploading..." : "Upload File"}
            </Button>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={(e) => handleFileSelect(e.target.files)}
            />
          </div>
        )}
      </div>

      {/* Tabs + Search + Filters */}
      <div className="flex flex-wrap items-center gap-3">
        <Tabs value={view} onValueChange={handleViewChange}>
          <TabsList>
            <TabsTrigger value="all">All Files</TabsTrigger>
            <TabsTrigger value="trash">Trash</TabsTrigger>
          </TabsList>
        </Tabs>
        <div className="relative min-w-0 flex-1">
          <Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
          <Input
            placeholder="Search files..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8"
          />
        </div>
        <Tabs
          value={typeFilter}
          onValueChange={(v) => {
            setTypeFilter(v);
            setPage(1);
          }}
        >
          <TabsList>
            <TabsTrigger value="all">All</TabsTrigger>
            <TabsTrigger value="image">Images</TabsTrigger>
            <TabsTrigger value="video">Videos</TabsTrigger>
            <TabsTrigger value="document">Docs</TabsTrigger>
          </TabsList>
        </Tabs>
      </div>

      {/* Drop overlay */}
      {dragOver && !isTrash && (
        <div className="flex items-center justify-center rounded-xl border-2 border-dashed border-primary bg-primary/5 p-12">
          <div className="text-center">
            <Upload className="mx-auto size-10 text-primary" />
            <p className="mt-2 font-medium text-primary">
              Drop files here to upload
            </p>
          </div>
        </div>
      )}

      {/* Content */}
      {isLoading ? (
        <div className="columns-2 gap-3 sm:columns-3 md:columns-4 lg:columns-5 xl:columns-6">
          {Array.from({ length: 12 }).map((_, i) => (
            <Skeleton
              key={i.toString()}
              className="mb-3 w-full rounded-lg"
              style={{ height: `${120 + (i % 3) * 60}px` }}
            />
          ))}
        </div>
      ) : !data?.items.length ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-16">
            {isTrash ? (
              <>
                <Trash2 className="mb-4 size-12 text-muted-foreground" />
                <p className="text-lg font-medium">Trash is empty</p>
                <p className="text-sm text-muted-foreground">
                  Deleted files will appear here
                </p>
              </>
            ) : (
              <>
                <ImagePlus className="mb-4 size-12 text-muted-foreground" />
                <p className="text-lg font-medium">No files yet</p>
                <p className="mb-4 text-sm text-muted-foreground">
                  Upload your first file to get started
                </p>
                <Button onClick={() => fileInputRef.current?.click()}>
                  <Upload data-icon="inline-start" />
                  Upload File
                </Button>
              </>
            )}
          </CardContent>
        </Card>
      ) : (
        <>
          {/* Masonry grid */}
          <div className="columns-2 gap-3 sm:columns-3 md:columns-4 lg:columns-5 xl:columns-6">
            {data.items.map((file) => (
              <FileCard
                key={file.id}
                file={file}
                isTrash={isTrash}
                selected={selectedIds.has(file.id)}
                hasAnySelection={hasSelection}
                onToggleSelect={(e) => toggleSelect(file.id, e)}
                onClick={() => {
                  setSelectedFile(file);
                  setDetailsOpen(true);
                }}
              />
            ))}
          </div>

          {/* Pagination */}
          {data.total > data.per_page && (
            <div className="flex items-center justify-between pt-2 text-sm text-muted-foreground">
              <span>
                Showing {data.items.length} of {data.total} files
              </span>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={page <= 1}
                  onClick={() => {
                    setPage((p) => p - 1);
                    setSelectedIds(new Set());
                  }}
                >
                  Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={data.items.length < data.per_page}
                  onClick={() => {
                    setPage((p) => p + 1);
                    setSelectedIds(new Set());
                  }}
                >
                  Next
                </Button>
              </div>
            </div>
          )}
        </>
      )}

      {/* Floating Action Bar */}
      {hasSelection && (
        <div className="animate-in fade-in-0 slide-in-from-bottom-2 fixed inset-x-0 bottom-6 z-50 mx-auto flex w-fit items-center gap-3 rounded-xl border bg-background/95 px-4 py-3 shadow-lg backdrop-blur supports-[backdrop-filter]:bg-background/80">
          <span className="text-sm font-medium">
            {selectedIds.size} selected
          </span>
          <div className="h-5 w-px bg-border" />
          {!isTrash ? (
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => setBatchDeleteConfirm(true)}
              disabled={batchDeleteMutation.isPending}
            >
              <Trash2 />
              {batchDeleteMutation.isPending ? "Deleting..." : "Move to Trash"}
            </Button>
          ) : (
            <>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleBatchRestore}
                disabled={batchRestoreMutation.isPending}
              >
                <RotateCcw />
                {batchRestoreMutation.isPending ? "Restoring..." : "Restore"}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="text-destructive hover:text-destructive"
                onClick={() => setBatchPermanentDeleteConfirm(true)}
                disabled={batchPermanentDeleteMutation.isPending}
              >
                <Trash2 />
                {batchPermanentDeleteMutation.isPending
                  ? "Deleting..."
                  : "Delete Forever"}
              </Button>
            </>
          )}
          <div className="h-5 w-px bg-border" />
          <button
            type="button"
            onClick={() => setSelectedIds(new Set())}
            className="text-muted-foreground transition-colors hover:text-foreground"
          >
            <X className="size-4" />
          </button>
        </div>
      )}

      {/* Details Dialog */}
      <Dialog open={detailsOpen} onOpenChange={setDetailsOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>File Details</DialogTitle>
          </DialogHeader>
          {selectedFile && (
            <div className="flex flex-col gap-4">
              {selectedFile.mime_type.startsWith("image/") ? (
                <img
                  src={selectedFile.url}
                  alt={selectedFile.original_name}
                  className="max-h-64 w-auto rounded-lg object-contain"
                />
              ) : selectedFile.mime_type.startsWith("video/") ? (
                <VideoPlayer
                  src={selectedFile.url}
                  poster={selectedFile.thumbnail_url || undefined}
                  className="w-full overflow-hidden rounded-lg"
                />
              ) : (
                <div className="flex h-32 items-center justify-center rounded-lg bg-muted">
                  <Badge variant="secondary">
                    {selectedFile.mime_type.split("/")[1]?.toUpperCase() ||
                      "FILE"}
                  </Badge>
                </div>
              )}
              <div className="grid grid-cols-2 gap-3 text-sm">
                <div>
                  <p className="text-muted-foreground">Name</p>
                  <p className="font-medium truncate">
                    {selectedFile.original_name}
                  </p>
                </div>
                <div>
                  <p className="text-muted-foreground">Size</p>
                  <p className="font-medium">
                    {formatFileSize(selectedFile.size)}
                  </p>
                </div>
                <div>
                  <p className="text-muted-foreground">Type</p>
                  <p className="font-medium">{selectedFile.mime_type}</p>
                </div>
                <div>
                  <p className="text-muted-foreground">Storage</p>
                  <p className="font-medium">{selectedFile.storage_provider}</p>
                </div>
                {selectedFile.width && selectedFile.height && (
                  <div>
                    <p className="text-muted-foreground">Dimensions</p>
                    <p className="font-medium">
                      {selectedFile.width} x {selectedFile.height}
                    </p>
                  </div>
                )}
                <div>
                  <p className="text-muted-foreground">Uploaded</p>
                  <p className="font-medium">
                    {formatDate(selectedFile.created_at)}
                  </p>
                </div>
                {isTrash && selectedFile.deleted_at && (
                  <div className="col-span-2">
                    <p className="text-muted-foreground">Deleted</p>
                    <p className="font-medium text-destructive">
                      {formatDate(selectedFile.deleted_at)}
                    </p>
                  </div>
                )}
              </div>
              <div className="flex flex-col gap-2">
                <p className="text-muted-foreground text-sm">URL</p>
                <code className="rounded bg-muted px-2 py-1 text-xs break-all">
                  {selectedFile.url}
                </code>
              </div>
            </div>
          )}
          <DialogFooter className="flex-row! justify-between!">
            {selectedFile && (
              <>
                {isTrash ? (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => restoreMutation.mutate(selectedFile.id)}
                    disabled={restoreMutation.isPending}
                  >
                    <RotateCcw />
                    {restoreMutation.isPending ? "Restoring..." : "Restore"}
                  </Button>
                ) : (
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={() => handleDelete(selectedFile)}
                    disabled={deleteMutation.isPending}
                  >
                    <Trash2 />
                    {deleteMutation.isPending ? "Deleting..." : "Move to Trash"}
                  </Button>
                )}
                <div className="flex gap-2">
                  {isTrash ? (
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => {
                        batchPermanentDeleteMutation.mutate([selectedFile.id]);
                        setDetailsOpen(false);
                      }}
                      disabled={batchPermanentDeleteMutation.isPending}
                    >
                      <Trash2 />
                      {batchPermanentDeleteMutation.isPending
                        ? "Deleting..."
                        : "Delete Forever"}
                    </Button>
                  ) : (
                    <>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => window.open(selectedFile.url, "_blank")}
                      >
                        <SquareArrowOutUpRight />
                        Open
                      </Button>
                      <a
                        href={selectedFile.url}
                        download={selectedFile.original_name}
                        className={buttonVariants({
                          size: "sm",
                          variant: "secondary",
                        })}
                      >
                        <Download className="size-4" />
                        Download
                      </a>
                    </>
                  )}
                </div>
              </>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Single-file delete reference warning */}
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

      {/* Batch delete confirmation */}
      <AlertDialog
        open={batchDeleteConfirm}
        onOpenChange={setBatchDeleteConfirm}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Move {selectedIds.size} file(s) to trash?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Files in trash can be restored later. They will be permanently
              deleted after 30 days.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={handleBatchDelete}
              disabled={batchDeleteMutation.isPending}
            >
              {batchDeleteMutation.isPending ? "Moving..." : "Move to Trash"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Batch permanent delete confirmation */}
      <AlertDialog
        open={batchPermanentDeleteConfirm}
        onOpenChange={setBatchPermanentDeleteConfirm}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Permanently delete {selectedIds.size} file(s)?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This action cannot be undone. These files will be permanently
              removed from storage and cannot be recovered.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={handleBatchPermanentDelete}
              disabled={batchPermanentDeleteMutation.isPending}
            >
              {batchPermanentDeleteMutation.isPending
                ? "Deleting..."
                : "Delete Forever"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  File Card                                                          */
/* ------------------------------------------------------------------ */

function FileCard({
  file,
  isTrash,
  selected,
  hasAnySelection,
  onToggleSelect,
  onClick,
}: {
  file: FileItem;
  isTrash: boolean;
  selected: boolean;
  hasAnySelection: boolean;
  onToggleSelect: (e: React.MouseEvent) => void;
  onClick: () => void;
}) {
  const isImage = file.mime_type.startsWith("image/");
  const isVideo = file.mime_type.startsWith("video/");
  const showIndicator = hasAnySelection || selected;

  return (
    <div
      className={`group/card relative mb-3 cursor-pointer overflow-hidden rounded-lg transition-all duration-150 ${
        selected
          ? "ring-2 ring-primary ring-offset-2 ring-offset-background"
          : "hover:shadow-lg"
      } ${isTrash ? "opacity-60" : ""}`}
    >
      {/* Main clickable area — opens details */}
      <button
        type="button"
        className="block w-full text-left"
        onClick={onClick}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onClick();
          }
        }}
      >
        {isImage ? (
          <img
            src={file.thumbnail_url || file.url}
            alt={file.original_name}
            className="block w-full"
            loading="lazy"
          />
        ) : isVideo ? (
          <div className="relative aspect-video bg-black">
            {file.thumbnail_url ? (
              <img
                src={file.thumbnail_url}
                alt={file.original_name}
                className="size-full object-cover"
                loading="lazy"
              />
            ) : (
              <div className="flex size-full items-center justify-center">
                <Badge variant="secondary">VIDEO</Badge>
              </div>
            )}
          </div>
        ) : (
          <div className="flex aspect-[3/4] flex-col items-center justify-center bg-muted p-3">
            <Badge variant="secondary">
              {file.mime_type.split("/")[1]?.toUpperCase() || "FILE"}
            </Badge>
            <p className="mt-2 line-clamp-2 text-center text-xs text-muted-foreground">
              {file.original_name}
            </p>
          </div>
        )}
      </button>

      {/* Bottom overlay — filename + size */}
      <div className="pointer-events-none absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 via-black/20 to-transparent px-2.5 pb-2 pt-8">
        <p className="truncate text-xs font-medium text-white">
          {file.original_name}
        </p>
        <div className="flex items-center justify-between">
          <p className="text-[10px] text-white/60">
            {formatFileSize(file.size)}
          </p>
          {isTrash && file.deleted_at && (
            <p className="text-[10px] text-white/60">
              {formatDeletedDate(file.deleted_at)}
            </p>
          )}
        </div>
      </div>

      {/* Selection circle — Google Photos style */}
      <button
        type="button"
        aria-label={selected ? "Deselect file" : "Select file"}
        className={`absolute left-2 top-2 z-10 flex size-7 items-center justify-center rounded-full border-2 transition-all duration-150 ${
          showIndicator
            ? "opacity-100"
            : "opacity-0 group-hover/card:opacity-100"
        } ${
          selected
            ? "border-primary bg-primary text-primary-foreground shadow-sm"
            : "border-white/80 bg-white/20 text-white shadow-sm backdrop-blur-sm hover:bg-white/40"
        }`}
        onClick={onToggleSelect}
      >
        {selected && <Check className="size-4" strokeWidth={3} />}
      </button>
    </div>
  );
}
