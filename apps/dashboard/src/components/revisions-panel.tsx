import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  History,
  Eye,
  RotateCcw,
  ChevronDown,
} from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { toast } from "sonner";
import { useForm } from "@tanstack/react-form";
import { z } from "zod";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
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
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { DynamicForm } from "@/components/dynamic-form";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import {
  getEntryRevisions,
  restoreEntryRevision,
  type EntryRevision,
  type SchemaDefinition,
} from "@/lib/api";

interface RevisionsPanelProps {
  entryId: string;
  siteId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  collectionDef: SchemaDefinition | null;
  onRestore: () => void;
}

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
  const [selectedRevision, setSelectedRevision] = useState<EntryRevision | null>(null);
  const [restoreTarget, setRestoreTarget] = useState<number | null>(null);
  const perPage = 20;

  const { data, isLoading } = useQuery({
    queryKey: ["entry-revisions", siteId, entryId, page],
    queryFn: () => getEntryRevisions(siteId, entryId, { page, per_page: perPage }),
    enabled: open,
  });

  const restoreMutation = useMutation({
    mutationFn: (revisionNumber: number) =>
      restoreEntryRevision(siteId, entryId, revisionNumber),
    onSuccess: () => {
      toast.success("Entry restored to previous version");
      queryClient.invalidateQueries({ queryKey: ["entry-revisions", siteId, entryId] });
      onRestore();
      setRestoreTarget(null);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const revisions = data?.items ?? [];
  const hasMore = data ? data.total > page * perPage : false;

  return (
    <>
      <Sheet open={open} onOpenChange={onOpenChange}>
        <SheetContent side="right" className="flex w-[380px] flex-col gap-0 p-0 sm:w-[520px]">
          <SheetHeader className="px-6 pt-6 pb-2">
            <SheetTitle className="flex items-center gap-2">
              <History className="size-5" />
              Revision History
            </SheetTitle>
            <SheetDescription>
              View and restore previous versions of this entry.
            </SheetDescription>
          </SheetHeader>

          <Separator />

          <ScrollArea className="flex-1">
            <div className="flex flex-col">
              {isLoading ? (
                <div className="flex flex-col gap-4 p-6">
                  {Array.from({ length: 5 }).map((_, i) => (
                    <Skeleton key={i} className="h-20 w-full" />
                  ))}
                </div>
              ) : revisions.length === 0 ? (
                <div className="flex flex-col items-center justify-center gap-2 p-12 text-center">
                  <History className="size-10 text-muted-foreground/50" />
                  <p className="text-sm text-muted-foreground">
                    No revisions yet.
                  </p>
                  <p className="text-xs text-muted-foreground">
                    Save changes to create the first revision.
                  </p>
                </div>
              ) : (
                revisions.map((rev) => (
                  <RevisionItem
                    key={rev.id}
                    revision={rev}
                    onPreview={() => setSelectedRevision(rev)}
                    onRestore={() => setRestoreTarget(rev.revision_number)}
                  />
                ))
              )}

              {hasMore && (
                <div className="flex justify-center p-4">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setPage((p) => p + 1)}
                  >
                    <ChevronDown data-icon="inline-start" />
                    Load more
                  </Button>
                </div>
              )}
            </div>
          </ScrollArea>
        </SheetContent>
      </Sheet>

      {/* Preview Dialog */}
      <Dialog
        open={!!selectedRevision}
        onOpenChange={(v) => !v && setSelectedRevision(null)}
      >
        <DialogContent className="max-h-[90vh] max-w-2xl overflow-y-auto">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Eye className="size-5" />
              Preview Revision {selectedRevision?.revision_number}
            </DialogTitle>
            <DialogDescription>
              This is a read-only view of revision {selectedRevision?.revision_number}.
            </DialogDescription>
          </DialogHeader>

          {selectedRevision && (
            <RevisionPreview
              revision={selectedRevision}
              collectionDef={collectionDef}
              siteId={siteId}
            />
          )}
        </DialogContent>
      </Dialog>

      {/* Restore Confirmation */}
      <AlertDialog
        open={restoreTarget !== null}
        onOpenChange={(v) => !v && setRestoreTarget(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2">
              <RotateCcw className="size-5" />
              Restore this version?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This will overwrite the current entry with revision {restoreTarget}. A new
              revision will be created so you can undo this later.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => setRestoreTarget(null)}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (restoreTarget !== null) {
                  restoreMutation.mutate(restoreTarget);
                }
              }}
              disabled={restoreMutation.isPending}
            >
              {restoreMutation.isPending ? "Restoring..." : "Restore"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function RevisionItem({
  revision,
  onPreview,
  onRestore,
}: {
  revision: EntryRevision;
  onPreview: () => void;
  onRestore: () => void;
}) {
  const date = new Date(revision.created_at);
  const timeAgo = formatDistanceToNow(date, { addSuffix: true });

  return (
    <div className="flex flex-col gap-2 px-6 py-4 border-b transition-colors hover:bg-muted/50">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="font-mono text-xs">
            Rev {revision.revision_number}
          </Badge>
          <span className="text-xs text-muted-foreground">{timeAgo}</span>
        </div>
      </div>

      {revision.change_summary && (
        <p className="text-sm text-foreground">{revision.change_summary}</p>
      )}

      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span>by {revision.created_by || "Unknown"}</span>
      </div>

      <div className="flex gap-2 mt-1">
        <Button variant="outline" size="sm" onClick={onPreview}>
          <Eye data-icon="inline-start" />
          Preview
        </Button>
        <Button variant="outline" size="sm" onClick={onRestore}>
          <RotateCcw data-icon="inline-start" />
          Restore
        </Button>
      </div>
    </div>
  );
}

function RevisionPreview({
  revision,
  collectionDef,
  siteId,
}: {
  revision: EntryRevision;
  collectionDef: SchemaDefinition | null;
  siteId: string;
}) {
  const parsedData =
    typeof revision.data === "string"
      ? JSON.parse(revision.data)
      : revision.data;

  const previewSchema = z.object({
    data: collectionDef
      ? buildPreviewSchema(collectionDef)
      : z.object({}),
    slug: z.string(),
  });

  const form = useForm({
    defaultValues: {
      data: parsedData as Record<string, unknown>,
      slug: "preview",
    },
    validators: {
      onSubmit: previewSchema,
    },
    onSubmit: async () => {},
  });

  return (
    <div className="flex flex-col gap-4 mt-4">
      <div className="rounded-md bg-muted/50 p-3 text-sm text-muted-foreground">
        <strong>Revision {revision.revision_number}</strong>
        {revision.change_summary && (
          <span> — {revision.change_summary}</span>
        )}
        <div className="mt-1 text-xs">
          {new Date(revision.created_at).toLocaleString()} by{" "}
          {revision.created_by || "Unknown"}
        </div>
      </div>

      {collectionDef ? (
        <DynamicForm
          fields={collectionDef.fields}
          form={form}
          prefix="data"
          siteId={siteId}
          readOnly
        />
      ) : (
        <pre className="rounded-md bg-muted p-4 text-xs overflow-auto">
          {JSON.stringify(parsedData, null, 2)}
        </pre>
      )}
    </div>
  );
}

function buildPreviewSchema(schema: SchemaDefinition) {
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
