import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Download, ImagePlus, Search, Trash2, Upload } from "lucide-react";
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
import {
  deleteMedia,
  getMedia,
  getMediaReferences,
  type Media,
  type MediaReference,
  uploadMedia,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/media")({
  component: MediaPage,
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

function MediaPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [page, setPage] = useState(1);
  const [selectedMedia, setSelectedMedia] = useState<Media | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<{
    media: Media;
    refs: MediaReference[];
  } | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const { data, isLoading } = useQuery({
    queryKey: ["media", siteId, page, search, typeFilter],
    queryFn: () =>
      getMedia(siteId, {
        page,
        search: search || undefined,
        type: typeFilter === "all" ? undefined : typeFilter,
      }),
  });

  const uploadMutation = useMutation({
    mutationFn: (file: File) => uploadMedia(siteId, file, "filesystem"),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["media", siteId] });
      toast.success("File uploaded successfully");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (mediaId: string) => deleteMedia(siteId, mediaId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["media", siteId] });
      setDetailsOpen(false);
      setSelectedMedia(null);
      toast.success("Media deleted");
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

  const handleDelete = async (media: Media) => {
    const refs = await getMediaReferences(siteId, media.id).catch(() => []);
    if (refs.length > 0) {
      setPendingDelete({ media, refs });
    } else {
      deleteMutation.mutate(media.id);
    }
  };

  const confirmDelete = () => {
    if (pendingDelete) {
      deleteMutation.mutate(pendingDelete.media.id);
      setPendingDelete(null);
    }
  };

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">Media Library</h1>
          <p className="text-sm text-muted-foreground">
            Upload and manage files for your site
          </p>
        </div>
        {/* biome-ignore lint/a11y/noStaticElementInteractions: drop zone */}
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
      </div>

      <div className="flex items-center gap-3">
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

      {dragOver && (
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
            <ImagePlus className="mb-4 size-12 text-muted-foreground" />
            <p className="text-lg font-medium">No media files yet</p>
            <p className="mb-4 text-sm text-muted-foreground">
              Upload your first file to get started
            </p>
            <Button onClick={() => fileInputRef.current?.click()}>
              <Upload data-icon="inline-start" />
              Upload File
            </Button>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4 md:grid-cols-4 lg:grid-cols-8">
            {data.items.map((media) => (
              <MediaCard
                key={media.id}
                media={media}
                onClick={() => {
                  setSelectedMedia(media);
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
        </>
      )}

      {/* Details Dialog */}
      <Dialog open={detailsOpen} onOpenChange={setDetailsOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Media Details</DialogTitle>
          </DialogHeader>
          {selectedMedia && (
            <div className="flex flex-col gap-4">
              {selectedMedia.mime_type.startsWith("image/") ? (
                <img
                  src={selectedMedia.url}
                  alt={selectedMedia.original_name}
                  className="max-h-64 w-auto rounded-lg object-contain"
                />
              ) : (
                <div className="flex h-32 items-center justify-center rounded-lg bg-muted">
                  <Badge variant="secondary">
                    {selectedMedia.mime_type.split("/")[1]?.toUpperCase() ||
                      "FILE"}
                  </Badge>
                </div>
              )}
              <div className="grid grid-cols-2 gap-3 text-sm">
                <div>
                  <p className="text-muted-foreground">Name</p>
                  <p className="font-medium truncate">
                    {selectedMedia.original_name}
                  </p>
                </div>
                <div>
                  <p className="text-muted-foreground">Size</p>
                  <p className="font-medium">
                    {formatFileSize(selectedMedia.size)}
                  </p>
                </div>
                <div>
                  <p className="text-muted-foreground">Type</p>
                  <p className="font-medium">{selectedMedia.mime_type}</p>
                </div>
                <div>
                  <p className="text-muted-foreground">Storage</p>
                  <p className="font-medium">
                    {selectedMedia.storage_provider}
                  </p>
                </div>
                {selectedMedia.width && selectedMedia.height && (
                  <div>
                    <p className="text-muted-foreground">Dimensions</p>
                    <p className="font-medium">
                      {selectedMedia.width} x {selectedMedia.height}
                    </p>
                  </div>
                )}
                <div>
                  <p className="text-muted-foreground">Uploaded</p>
                  <p className="font-medium">
                    {formatDate(selectedMedia.created_at)}
                  </p>
                </div>
              </div>
              <div className="flex flex-col gap-2">
                <p className="text-muted-foreground text-sm">URL</p>
                <code className="rounded bg-muted px-2 py-1 text-xs break-all">
                  {selectedMedia.url}
                </code>
              </div>
            </div>
          )}
          <DialogFooter>
            {selectedMedia && (
              <>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => window.open(selectedMedia.url, "_blank")}
                >
                  <Download data-icon="inline-start" />
                  Open
                </Button>
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() => selectedMedia && handleDelete(selectedMedia)}
                  disabled={deleteMutation.isPending}
                >
                  <Trash2 data-icon="inline-start" />
                  {deleteMutation.isPending ? "Deleting..." : "Delete"}
                </Button>
              </>
            )}
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
            <AlertDialogTitle>Delete media?</AlertDialogTitle>
            <AlertDialogDescription>
              This media is used in {pendingDelete?.refs.length} content item(s)
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
    </div>
  );
}

function MediaCard({ media, onClick }: { media: Media; onClick: () => void }) {
  const isImage = media.mime_type.startsWith("image/");

  return (
    <button
      type="button"
      className="group relative aspect-square cursor-pointer overflow-hidden rounded-lg border text-left transition-shadow hover:shadow-md"
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
      <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/60 to-transparent p-2">
        <p className="truncate text-xs text-white">{media.original_name}</p>
        <p className="text-[10px] text-white/70">
          {formatFileSize(media.size)}
        </p>
      </div>
    </button>
  );
}
