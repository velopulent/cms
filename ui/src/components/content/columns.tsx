import { Link } from "@tanstack/react-router";
import type { ColumnDef } from "@tanstack/react-table";
import { ArrowUpDown, Globe, GlobeLock, Pencil, Trash2 } from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import type { Content } from "@/lib/api";

function extractTitle(item: Content): string {
  try {
    const parsedData =
      typeof item.data === "string" ? JSON.parse(item.data) : item.data;
    return (
      (parsedData.title as string) || (parsedData.name as string) || item.slug
    );
  } catch {
    return item.slug;
  }
}

interface CreateColumnsParams {
  siteId: string;
  collectionSlug: string;
  onPublish: (id: string) => void;
  onUnpublish: (id: string) => void;
  onDelete: (id: string) => void;
  isPublishPending: boolean;
  isUnpublishPending: boolean;
  isDeletePending: boolean;
}

export function createColumns({
  siteId,
  collectionSlug,
  onPublish,
  onUnpublish,
  onDelete,
  isPublishPending,
  isUnpublishPending,
  isDeletePending,
}: CreateColumnsParams): ColumnDef<Content>[] {
  return [
    {
      accessorKey: "data",
      header: ({ column }) => (
        <Button
          variant="ghost"
          onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
        >
          Title
          <ArrowUpDown />
        </Button>
      ),
      cell: ({ row }) => {
        const title = extractTitle(row.original);
        return <span className="font-medium">{title}</span>;
      },
      sortingFn: (rowA, rowB) => {
        const titleA = extractTitle(rowA.original).toLowerCase();
        const titleB = extractTitle(rowB.original).toLowerCase();
        return titleA.localeCompare(titleB);
      },
    },
    {
      accessorKey: "slug",
      header: "Slug",
      cell: ({ row }) => <Badge variant="outline">{row.original.slug}</Badge>,
    },
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ row }) => (
        <Badge
          variant={
            row.original.status === "published" ? "default" : "secondary"
          }
        >
          {row.original.status}
        </Badge>
      ),
    },
    {
      accessorKey: "updated_at",
      header: ({ column }) => (
        <Button
          variant="ghost"
          onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
        >
          Updated
          <ArrowUpDown />
        </Button>
      ),
      cell: ({ row }) => (
        <span className="text-sm text-muted-foreground">
          {new Date(row.original.updated_at).toLocaleDateString()}
        </span>
      ),
    },
    {
      id: "actions",
      header: () => <div className="text-right">Actions</div>,
      cell: ({ row }) => {
        const item = row.original;
        const title = extractTitle(item);

        return (
          <div className="flex justify-end gap-1">
            <Link
              to="/sites/$siteId/content/$collectionSlug/$id/edit"
              params={{ siteId, collectionSlug, id: item.id }}
              className={buttonVariants({
                variant: "ghost",
                size: "icon",
              })}
            >
              <Pencil />
            </Link>
            {item.status === "draft" ? (
              <Button
                variant="ghost"
                size="icon"
                onClick={() => onPublish(item.id)}
                disabled={isPublishPending}
              >
                <Globe />
              </Button>
            ) : (
              <Button
                variant="ghost"
                size="icon"
                onClick={() => onUnpublish(item.id)}
                disabled={isUnpublishPending}
              >
                <GlobeLock />
              </Button>
            )}
            <AlertDialog>
              <AlertDialogTrigger
                render={<Button variant="ghost" size="icon" />}
              >
                <Trash2 />
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Delete content?</AlertDialogTitle>
                  <AlertDialogDescription>
                    This will permanently delete &quot;{title}&quot;. This
                    action cannot be undone.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => onDelete(item.id)}
                    disabled={isDeletePending}
                  >
                    Delete
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </div>
        );
      },
    },
  ];
}
