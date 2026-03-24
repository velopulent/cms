import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Globe, GlobeLock, Pencil, Plus, Search, Trash2 } from "lucide-react";
import { useState } from "react";
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
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  type Content,
  deleteContent,
  getContent,
  getContentType,
  publishContent,
  unpublishContent,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/content/$typeSlug/")({
  component: ContentListPage,
});

function ContentListPage() {
  const { typeSlug } = Route.useParams();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("");

  const { data: contentType, isLoading: typeLoading } = useQuery({
    queryKey: ["content-type", typeSlug],
    queryFn: () => getContentType(typeSlug),
  });

  const { data: items, isLoading: itemsLoading } = useQuery({
    queryKey: ["content", typeSlug, statusFilter, search],
    queryFn: () =>
      getContent({
        type: typeSlug,
        status: statusFilter || undefined,
        search: search || undefined,
      }),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteContent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["content", typeSlug] });
      toast.success("Content deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const publishMutation = useMutation({
    mutationFn: publishContent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["content", typeSlug] });
      toast.success("Published");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const unpublishMutation = useMutation({
    mutationFn: unpublishContent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["content", typeSlug] });
      toast.success("Unpublished");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const isLoading = typeLoading || itemsLoading;
  const typeName = contentType?.name ?? typeSlug;

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">{typeName}</h1>
          <p className="text-sm text-muted-foreground">
            Manage your {typeName.toLowerCase()} content
          </p>
        </div>
        <Button
          render={<Link to="/content/$typeSlug/new" params={{ typeSlug }} />}
        >
          <Plus data-icon="inline-start" />
          New {typeName}
        </Button>
      </div>

      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="Search..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-9"
          />
        </div>
        <Select
          items={[
            { label: "All statuses", value: null },
            { label: "Draft", value: "draft" },
            { label: "Published", value: "published" },
          ]}
          value={statusFilter}
          onValueChange={(val) => setStatusFilter((val as string) ?? "")}
        >
          <SelectTrigger className="w-[150px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value="">All statuses</SelectItem>
              <SelectItem value="draft">Draft</SelectItem>
              <SelectItem value="published">Published</SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>

      {isLoading ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
          <Skeleton className="h-12 w-full" />
        </div>
      ) : !items?.length ? (
        <div className="flex flex-col items-center justify-center py-12">
          <p className="text-lg font-medium">No content yet</p>
          <p className="text-sm text-muted-foreground">
            Create your first {typeName.toLowerCase()} to get started.
          </p>
        </div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Title</TableHead>
              <TableHead>Slug</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Updated</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {items.map((item: Content) => {
              let title: string;
              try {
                const data = JSON.parse(item.data);
                title = data.title || data.name || item.slug;
              } catch {
                title = item.slug;
              }

              return (
                <TableRow key={item.id}>
                  <TableCell className="font-medium">{title}</TableCell>
                  <TableCell>
                    <Badge variant="outline">{item.slug}</Badge>
                  </TableCell>
                  <TableCell>
                    <Badge
                      variant={
                        item.status === "published" ? "default" : "secondary"
                      }
                    >
                      {item.status}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-sm text-muted-foreground">
                    {new Date(item.updated_at).toLocaleDateString()}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="ghost"
                        size="icon"
                        render={
                          <Link
                            to="/content/$typeSlug/$id/edit"
                            params={{ typeSlug, id: String(item.id) }}
                          />
                        }
                      >
                        <Pencil />
                      </Button>
                      {item.status === "draft" ? (
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => publishMutation.mutate(item.id)}
                          disabled={publishMutation.isPending}
                        >
                          <Globe />
                        </Button>
                      ) : (
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => unpublishMutation.mutate(item.id)}
                          disabled={unpublishMutation.isPending}
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
                              This will permanently delete &quot;{title}&quot;.
                              This action cannot be undone.
                            </AlertDialogDescription>
                          </AlertDialogHeader>
                          <AlertDialogFooter>
                            <AlertDialogCancel>Cancel</AlertDialogCancel>
                            <AlertDialogAction
                              onClick={() => deleteMutation.mutate(item.id)}
                            >
                              Delete
                            </AlertDialogAction>
                          </AlertDialogFooter>
                        </AlertDialogContent>
                      </AlertDialog>
                    </div>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
