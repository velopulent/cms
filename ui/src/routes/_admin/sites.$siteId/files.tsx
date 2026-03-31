import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  Download,
  ImagePlus,
  RotateCcw,
  Search,
  SquareArrowOutUpRight,
  Trash2,
  Upload,
  X,
} from "lucide-react";
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
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
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

function FilesPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
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
    queryKey: ["files", siteId, page, search, typeFilter, view],
    queryFn: () =>
      getFiles(siteId, {
        page,
        search: search || undefined,
        type: typeFilter === "all" ? undefined : typeFilter,
        trashed: isTrash,
      }),
  });

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
      toast.success("File deleted");
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
      toast.success(`${ids.length} file(s) deleted`);
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

  const toggleSelect = (fileId: string) => {
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

  const toggleSelectAll = () => {
    if (!data?.items) return;
    if (selectedIds.size === data.items.length) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(data.items.map((f) => f.id)));
    }
  };

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

  const hasSelection = selectedIds.size > 0;
  const allSelected =
    data?.items && data.items.length > 0 && selectedIds.size === data.items.length;

  // Clear selection when view or page changes
  const handleViewChange = (v: string) => {
    setView(v as "all" | "trash");
    setSelectedIds(new Set());
    setPage(1);
  };

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Files</h1>
          <p className="text-sm text-muted-foreground">
            Upload and manage files for your site
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

      <div className="flex items-center gap-3">
        <Tabs value={view} onValueChange={handleViewChange}>
          <TabsList>
            <TabsTrigger value="all">All Files</TabsTrigger>
            <TabsTrigger value="trash">Trash</TabsTrigger>
          </TabsList>
        </Tabs>
        <div className="relative flex-1">
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
            <TabsTrigger value="document">Documents</TabsTrigger>
          </TabsList>
        </Tabs>
      </div>

      {dragOver && !isTrash && (
        <div className="flex items-center justify-center rounded-lg border-2 border-dashed border-primary bg-primary/5 p-8">
          <div className="text-center">
            <Upload className="mx-auto size-8 text-primary" />
            <p className="mt-2 font-medium text-primary">
              Drop files here to upload
            </p>
          </div>
        </div>
      )}

      {isLoading ? (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
          {["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"].map((id) => (
            <Skeleton key={id} className="aspect-square rounded-lg" />
          ))}
        </div>
      ) : !data?.items.length ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-16">
            {isTrash ? (
              <>
                <Trash2 className="mb-4 size-12 text-muted-foreground" />
                <p className="text-lg font-medium">Trash is empty</p>
                <p className="mb-4 text-sm text-muted-foreground">
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
          {/* Selection toolbar */}
          <div className="flex items-center justify-between rounded-lg border bg-muted/30 px-4 py-2">
            <div className="flex items-center gap-3">
              <Checkbox
                checked={allSelected}
                onCheckedChange={toggleSelectAll}
              />
              <span className="text-sm text-muted-foreground">
                {hasSelection
                  ? `${selectedIds.size} of ${data.items.length} selected`
                  : `${data.items.length} file(s)`}
              </span>
            </div>
            {hasSelection && (
              <div className="flex items-center gap-2">
                {!isTrash ? (
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={() => setBatchDeleteConfirm(true)}
                    disabled={batchDeleteMutation.isPending}
                  >
                    <Trash2 />
                    {batchDeleteMutation.isPending
                      ? "Deleting..."
                      : `Delete ${selectedIds.size}`}
                  </Button>
                ) : (
                  <>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={handleBatchRestore}
                      disabled={batchRestoreMutation.isPending}
                    >
                      <RotateCcw />
                      {batchRestoreMutation.isPending
                        ? "Restoring..."
                        : `Restore ${selectedIds.size}`}
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => setBatchPermanentDeleteConfirm(true)}
                      disabled={batchPermanentDeleteMutation.isPending}
                    >
                      <Trash2 />
                      {batchPermanentDeleteMutation.isPending
                        ? "Deleting..."
                        : `Delete ${selectedIds.size}`}
                    </Button>
                  </>
                )}
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setSelectedIds(new Set())}
                >
                  <X />
                  Clear
                </Button>
              </div>
            )}
          </div>

          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4 md:grid-cols-4 lg:grid-cols-8">
            {data.items.map((file) => (
              <FileCard
                key={file.id}
                file={file}
                isTrash={isTrash}
                selected={selectedIds.has(file.id)}
                onToggleSelect={() => toggleSelect(file.id)}
                onClick={() => {
                  setSelectedFile(file);
                  setDetailsOpen(true);
                }}
              />
            ))}
          </div>

          {data.total > data.per_page && (
            <div className="flex items-center justify-between text-sm text-muted-foreground">
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
                    onClick={() => {
                      restoreMutation.mutate(selectedFile.id);
                    }}
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
                    {deleteMutation.isPending ? "Deleting..." : "Delete"}
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
                        : "Permanently Delete"}
                    </Button>
                  ) : (
                    <>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() =>
                          window.open(selectedFile.url, "_blank")
                        }
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
              Delete {selectedIds.size} file(s)?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This will move {selectedIds.size} file(s) to the trash. You can
              restore them later from the Trash tab.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={handleBatchDelete}
              disabled={batchDeleteMutation.isPending}
            >
              {batchDeleteMutation.isPending ? "Deleting..." : "Delete"}
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
              This action cannot be undone. {selectedIds.size} file(s) will be
              permanently removed from storage and cannot be recovered.
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
                : "Permanently Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function FileCard({
  file,
  isTrash,
  selected,
  onToggleSelect,
  onClick,
}: {
  file: FileItem;
  isTrash: boolean;
  selected: boolean;
  onToggleSelect: () => void;
  onClick: () => void;
}) {
  const isImage = file.mime_type.startsWith("image/");
  const isVideo = file.mime_type.startsWith("video/");

  return (
    <div
      className={`group relative aspect-square cursor-pointer overflow-hidden rounded-lg border text-left transition-shadow hover:shadow-md ${
        selected ? "ring-2 ring-primary" : ""
      } ${isTrash ? "opacity-70" : ""}`}
    >
      <button
        type="button"
        className="size-full"
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
        <div className="absolute bottom-0 left-0 right-0 bg-linear-to-t from-black/60 to-transparent p-2">
          <p className="truncate text-xs text-white">{file.original_name}</p>
          <p className="text-[10px] text-white/70">
            {formatFileSize(file.size)}
          </p>
        </div>
      </button>
      <button
        type="button"
        className="absolute top-2 left-2 z-10 flex size-5 items-center justify-center rounded bg-background/80 shadow-sm transition-opacity group-hover:opacity-100"
        style={{ opacity: selected ? 1 : undefined }}
        onClick={(e) => {
          e.stopPropagation();
          onToggleSelect();
        }}
      >
        <Checkbox
          checked={selected}
          onCheckedChange={() => onToggleSelect()}
          className="size-3.5"
        />
      </button>
    </div>
  );
}
