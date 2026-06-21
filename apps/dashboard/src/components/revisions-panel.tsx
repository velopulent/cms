import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import {
  AlertTriangle,
  ChevronDown,
  Clock,
  Eye,
  History,
  Loader2,
  RotateCcw,
  User,
} from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { DynamicForm } from "@/components/dynamic-form";
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
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  type Entry,
  type EntryRevision,
  getEntryRevisions,
  restoreEntryRevision,
  type SchemaDefinition,
} from "@/lib/api";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PER_PAGE = 20;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface RevisionsPanelProps {
  entryId: string;
  siteId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  collectionDef: SchemaDefinition | null;
  onRestore: (restored: Entry) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Safely parses revision data that may arrive as a JSON string or a plain
 * object (depending on the API serialiser). Returns `null` on failure so the
 * caller can render a graceful fallback.
 */
function parseRevisionData(data: unknown): Record<string, unknown> | null {
  if (typeof data === "object" && data !== null) {
    return data as Record<string, unknown>;
  }
  if (typeof data === "string") {
    try {
      return JSON.parse(data) as Record<string, unknown>;
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Builds a lenient Zod schema for read-only preview of a revision.
 * Every field is optional so missing data doesn't cause validation noise.
 */
function buildPreviewSchema(
  schema: SchemaDefinition,
): z.ZodObject<Record<string, z.ZodTypeAny>> {
  const shape: Record<string, z.ZodTypeAny> = {};
  for (const f of schema.fields) {
    switch (f.type) {
      case "number":
        shape[f.name] = z.number().optional();
        break;
      case "boolean":
        shape[f.name] = z.boolean().optional();
        break;
      default:
        shape[f.name] = z.string().optional();
    }
  }
  return z.object(shape);
}

// ---------------------------------------------------------------------------
// RevisionsPanel
// ---------------------------------------------------------------------------

export function RevisionsPanel({
  entryId,
  siteId,
  open,
  onOpenChange,
  collectionDef,
  onRestore,
}: RevisionsPanelProps) {
  const queryClient = useQueryClient();

  const [page, setPage] = useState(1);
  const [selectedRevision, setSelectedRevision] =
    useState<EntryRevision | null>(null);
  const [restoreTarget, setRestoreTarget] = useState<number | null>(null);

  const { data, isLoading, isError } = useQuery({
    queryKey: ["entry-revisions", siteId, entryId, page],
    queryFn: () =>
      getEntryRevisions(siteId, entryId, { page, per_page: PER_PAGE }),
    enabled: open,
    // Keep previous page data visible while the next page loads.
    placeholderData: (prev) => prev,
  });

  const restoreMutation = useMutation({
    mutationFn: (revisionNumber: number) =>
      restoreEntryRevision(siteId, entryId, revisionNumber),
    onSuccess: (restored) => {
      toast.success("Entry restored to previous version");
      queryClient.invalidateQueries({
        queryKey: ["entry-revisions", siteId, entryId],
      });
      setRestoreTarget(null);
      onRestore(restored);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const revisions = data?.items ?? [];
  const hasMore = data ? data.total > page * PER_PAGE : false;

  const handleRestoreConfirm = useCallback(() => {
    if (restoreTarget !== null) {
      restoreMutation.mutate(restoreTarget);
    }
  }, [restoreTarget, restoreMutation]);

  // Reset page when the panel is closed so it opens fresh next time.
  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (!next) setPage(1);
      onOpenChange(next);
    },
    [onOpenChange],
  );

  return (
    <TooltipProvider>
      {/* ------------------------------------------------------------------ */}
      {/* Revisions sheet                                                      */}
      {/* ------------------------------------------------------------------ */}
      <Sheet open={open} onOpenChange={handleOpenChange}>
        <SheetContent
          side="right"
          className="flex w-full flex-col gap-0 p-0 sm:w-[420px] sm:max-w-[420px]"
        >
          {/* Header */}
          <SheetHeader className="px-5 pb-3 pt-5">
            <SheetTitle className="flex items-center gap-2 text-base">
              <span className="flex size-7 items-center justify-center rounded-md bg-primary/10 text-primary">
                <History className="size-4" />
              </span>
              Revision History
            </SheetTitle>
            <SheetDescription className="text-xs">
              View and restore previous versions of this entry.
            </SheetDescription>
          </SheetHeader>

          <Separator />

          {/* Body */}
          <ScrollArea className="flex-1">
            {isLoading ? (
              <RevisionListSkeleton />
            ) : isError ? (
              <RevisionListError />
            ) : revisions.length === 0 ? (
              <RevisionListEmpty />
            ) : (
              <div className="flex flex-col py-1">
                {revisions.map((rev, index) => (
                  <RevisionItem
                    key={rev.id}
                    revision={rev}
                    isLatest={index === 0 && page === 1}
                    onPreview={() => setSelectedRevision(rev)}
                    onRestore={() => setRestoreTarget(rev.revision_number)}
                  />
                ))}

                {hasMore && (
                  <div className="flex justify-center px-5 py-3">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="w-full text-muted-foreground hover:text-foreground"
                      onClick={() => setPage((p) => p + 1)}
                      disabled={isLoading}
                    >
                      <ChevronDown className="mr-1.5 size-3.5" />
                      Load older revisions
                    </Button>
                  </div>
                )}
              </div>
            )}
          </ScrollArea>
        </SheetContent>
      </Sheet>

      {/* ------------------------------------------------------------------ */}
      {/* Preview dialog                                                       */}
      {/* ------------------------------------------------------------------ */}
      <Dialog
        open={!!selectedRevision}
        onOpenChange={(v) => {
          if (!v) setSelectedRevision(null);
        }}
      >
        <DialogContent
          className={cn(
            "flex h-[90dvh] w-full flex-col gap-0 overflow-hidden p-0",
            "sm:max-w-3xl md:max-w-4xl lg:max-w-5xl",
          )}
        >
          <DialogHeader className="shrink-0 gap-1.5 px-6 pb-4 pt-6">
            <div className="flex items-center gap-2">
              <span className="flex size-7 items-center justify-center rounded-md bg-blue-500/10 text-blue-600 dark:text-blue-400">
                <Eye className="size-4" />
              </span>
              <DialogTitle className="text-base">
                Revision {selectedRevision?.revision_number}
              </DialogTitle>
              <Badge
                variant="secondary"
                className="ml-auto font-normal text-xs"
              >
                Read-only preview
              </Badge>
            </div>
            <DialogDescription className="text-xs">
              {selectedRevision?.change_summary
                ? `"${selectedRevision.change_summary}"`
                : "No change summary provided for this revision."}
            </DialogDescription>
            <div className="flex items-center gap-3 pt-0.5 text-[11px] text-muted-foreground">
              <span className="flex items-center gap-1">
                <Clock className="size-3" />
                {selectedRevision
                  ? new Date(selectedRevision.created_at).toLocaleString()
                  : ""}
              </span>
              <span className="flex items-center gap-1">
                <User className="size-3" />
                {selectedRevision?.created_by || "Unknown"}
              </span>
            </div>
          </DialogHeader>

          <Separator className="shrink-0" />

          {/* Scroll only the form content, not the header or footer */}
          <ScrollArea className="min-h-0 flex-1 overscroll-contain">
            <div className="px-6 py-4">
              {selectedRevision && (
                /* Key forces useForm to reinitialise when the revision changes */
                <RevisionPreview
                  key={selectedRevision.id}
                  revision={selectedRevision}
                  collectionDef={collectionDef}
                  siteId={siteId}
                />
              )}
            </div>
          </ScrollArea>

          <DialogFooter className="mx-0 mb-0 shrink-0 gap-2 rounded-none border-t bg-muted/50 px-6 py-3">
            <DialogClose render={<Button variant="outline">Close</Button>}>
              Close
            </DialogClose>
            {selectedRevision && (
              <Button
                onClick={() => {
                  const n = selectedRevision.revision_number;
                  setSelectedRevision(null);
                  setRestoreTarget(n);
                }}
              >
                <RotateCcw data-icon="inline-start" />
                Restore this revision
              </Button>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* ------------------------------------------------------------------ */}
      {/* Restore confirmation                                                 */}
      {/* ------------------------------------------------------------------ */}
      <AlertDialog
        open={restoreTarget !== null}
        onOpenChange={(v) => {
          if (!v) setRestoreTarget(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2">
              <span className="flex size-7 items-center justify-center rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400">
                <RotateCcw className="size-4" />
              </span>
              Restore revision {restoreTarget}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              The current entry will be overwritten with the content from
              revision <strong>{restoreTarget}</strong>. A new revision will be
              saved first, so you can always undo this action.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={restoreMutation.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={handleRestoreConfirm}
              disabled={restoreMutation.isPending}
            >
              {restoreMutation.isPending ? (
                <>
                  <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                  Restoring…
                </>
              ) : (
                "Restore"
              )}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </TooltipProvider>
  );
}

// ---------------------------------------------------------------------------
// List states
// ---------------------------------------------------------------------------

function RevisionListSkeleton() {
  const keys = useMemo(() => Array.from({ length: 5 }, (_, i) => i), []);
  return (
    <div className="flex flex-col gap-px py-1">
      {keys.map((k) => (
        <div key={k} className="px-5 py-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Skeleton className="h-5 w-14 rounded-md" />
              <Skeleton className="h-4 w-24 rounded" />
            </div>
            <div className="flex gap-1.5">
              <Skeleton className="h-7 w-16 rounded-md" />
              <Skeleton className="h-7 w-16 rounded-md" />
            </div>
          </div>
          <Skeleton className="mt-2.5 h-4 w-3/4 rounded" />
          <Skeleton className="mt-1.5 h-3 w-1/3 rounded" />
        </div>
      ))}
    </div>
  );
}

function RevisionListEmpty() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-16 text-center">
      <span className="flex size-12 items-center justify-center rounded-xl bg-muted text-muted-foreground">
        <History className="size-6" />
      </span>
      <div>
        <p className="text-sm font-medium text-foreground">No revisions yet</p>
        <p className="mt-1 text-xs text-muted-foreground">
          Save changes to create the first revision.
        </p>
      </div>
    </div>
  );
}

function RevisionListError() {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-16 text-center">
      <span className="flex size-12 items-center justify-center rounded-xl bg-destructive/10 text-destructive">
        <AlertTriangle className="size-6" />
      </span>
      <div>
        <p className="text-sm font-medium text-foreground">
          Failed to load revisions
        </p>
        <p className="mt-1 text-xs text-muted-foreground">
          Please close and reopen the panel to try again.
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// RevisionItem
// ---------------------------------------------------------------------------

interface RevisionItemProps {
  revision: EntryRevision;
  isLatest: boolean;
  onPreview: () => void;
  onRestore: () => void;
}

function RevisionItem({
  revision,
  isLatest,
  onPreview,
  onRestore,
}: RevisionItemProps) {
  // Memoised so the date calculation doesn't run on every parent render.
  const timeAgo = useMemo(
    () =>
      formatDistanceToNow(new Date(revision.created_at), { addSuffix: true }),
    [revision.created_at],
  );

  const fullDate = useMemo(
    () => new Date(revision.created_at).toLocaleString(),
    [revision.created_at],
  );

  return (
    <div className="group relative px-5 py-3.5 transition-colors hover:bg-muted/40">
      {/* Subtle left accent line on hover */}
      <span className="absolute inset-y-0 left-0 w-[3px] rounded-r-full bg-primary opacity-0 transition-opacity group-hover:opacity-100" />

      {/* Row 1: revision badge + time + actions */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Badge
            variant="outline"
            className="shrink-0 font-mono text-[11px] tabular-nums"
          >
            #{revision.revision_number}
          </Badge>
          {isLatest && (
            <Badge className="shrink-0 bg-emerald-500/15 text-[10px] text-emerald-700 hover:bg-emerald-500/15 dark:text-emerald-400">
              Latest
            </Badge>
          )}
          <Tooltip>
            <TooltipTrigger>
              <span className="flex min-w-0 items-center gap-1 truncate text-xs text-muted-foreground">
                <Clock className="size-3 shrink-0" />
                <span className="truncate">{timeAgo}</span>
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs">
              {fullDate}
            </TooltipContent>
          </Tooltip>
        </div>

        {/* Action buttons — hidden on the latest revision (no-op / redundant) */}
        {!isLatest && (
          <div className="flex shrink-0 items-center gap-1">
            <Tooltip>
              <TooltipTrigger>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 gap-1.5 px-2 text-xs"
                  onClick={onPreview}
                >
                  <Eye className="size-3.5" />
                  <span className="hidden sm:inline">Preview</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="text-xs">
                Preview this revision
              </TooltipContent>
            </Tooltip>

            <Tooltip>
              <TooltipTrigger>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 gap-1.5 px-2 text-xs text-muted-foreground hover:text-foreground"
                  onClick={onRestore}
                >
                  <RotateCcw className="size-3.5" />
                  <span className="hidden sm:inline">Restore</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="text-xs">
                Restore to this revision
              </TooltipContent>
            </Tooltip>
          </div>
        )}
      </div>

      {/* Row 2: change summary */}
      {revision.change_summary && (
        <p className="mt-1.5 line-clamp-2 text-xs text-foreground/80">
          {revision.change_summary}
        </p>
      )}

      {/* Row 3: author */}
      <div className="mt-1.5 flex items-center gap-1 text-[11px] text-muted-foreground">
        <User className="size-3 shrink-0" />
        <span className="truncate">{revision.created_by || "Unknown"}</span>
      </div>

      <Separator className="mt-3.5" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// RevisionPreview
// ---------------------------------------------------------------------------

interface RevisionPreviewProps {
  revision: EntryRevision;
  collectionDef: SchemaDefinition | null;
  siteId: string;
}

function RevisionPreview({
  revision,
  collectionDef,
  siteId,
}: RevisionPreviewProps) {
  const parsedData = useMemo(
    () => parseRevisionData(revision.data),
    [revision.data],
  );

  // Memoised so Zod schema isn't rebuilt on every render.
  const previewSchema = useMemo(
    () =>
      z.object({
        data: collectionDef
          ? buildPreviewSchema(collectionDef)
          : z.record(z.string(), z.unknown()),
        slug: z.string(),
      }),
    [collectionDef],
  );

  const form = useForm({
    defaultValues: {
      data: (parsedData ?? {}) as Record<string, unknown>,
      slug: "preview",
    },
    validators: { onSubmit: previewSchema },
    onSubmit: async () => {
      // Read-only form — submission is intentionally a no-op.
    },
  });

  if (parsedData === null) {
    return (
      <div className="flex flex-col items-center gap-3 rounded-lg border border-destructive/30 bg-destructive/5 p-6 text-center">
        <AlertTriangle className="size-8 text-destructive/70" />
        <p className="text-sm text-destructive">
          Could not parse revision data. The stored content may be malformed.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Form or raw JSON fallback */}
      {collectionDef ? (
        <DynamicForm
          fields={collectionDef.fields}
          form={form}
          prefix="data"
          siteId={siteId}
          readOnly
        />
      ) : (
        <pre className="overflow-auto rounded-lg border bg-muted/50 p-4 text-xs leading-relaxed">
          {JSON.stringify(parsedData, null, 2)}
        </pre>
      )}
    </div>
  );
}
