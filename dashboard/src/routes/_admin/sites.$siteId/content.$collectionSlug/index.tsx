import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { buttonVariants } from "@/components/ui/button";
import { DataTable } from "@/components/ui/data-table";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  deleteContent,
  getCollection,
  getContent,
  publishContent,
  unpublishContent,
} from "@/lib/api";
import { createColumns } from "@/components/content/columns";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/content/$collectionSlug/",
)({
  component: ContentListPage,
});

function ContentListPage() {
  const { siteId, collectionSlug } = Route.useParams();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  const { data: collection, isLoading: collectionLoading } = useQuery({
    queryKey: ["collection", siteId, collectionSlug],
    queryFn: () => getCollection(siteId, collectionSlug),
  });

  const { data: contentResponse, isLoading: itemsLoading } = useQuery({
    queryKey: ["content", siteId, collectionSlug, statusFilter, search, page, pageSize],
    queryFn: () =>
      getContent(siteId, {
        type: collectionSlug,
        status: statusFilter || undefined,
        search: search || undefined,
        page,
        pageSize,
      }),
  });

  const items = contentResponse?.items ?? [];
  const total = contentResponse?.total ?? 0;

  const handleSearchChange = (value: string | null) => {
    setSearch(value || "");
    setPage(1);
  };

  const handleStatusChange = (value: string | null) => {
    setStatusFilter(value || "");
    setPage(1);
  };

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteContent(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["content", siteId, collectionSlug],
      });
      toast.success("Content deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const publishMutation = useMutation({
    mutationFn: (id: string) => publishContent(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["content", siteId, collectionSlug],
      });
      toast.success("Published");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const unpublishMutation = useMutation({
    mutationFn: (id: string) => unpublishContent(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["content", siteId, collectionSlug],
      });
      toast.success("Unpublished");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const isLoading = collectionLoading || itemsLoading;
  const collectionName = collection?.name ?? collectionSlug;

  const columns = useMemo(
    () =>
      createColumns({
        siteId,
        collectionSlug,
        onPublish: publishMutation.mutate,
        onUnpublish: unpublishMutation.mutate,
        onDelete: deleteMutation.mutate,
        isPublishPending: publishMutation.isPending,
        isUnpublishPending: unpublishMutation.isPending,
        isDeletePending: deleteMutation.isPending,
      }),
    [
      siteId,
      collectionSlug,
      publishMutation.mutate,
      publishMutation.isPending,
      unpublishMutation.mutate,
      unpublishMutation.isPending,
      deleteMutation.mutate,
      deleteMutation.isPending,
    ],
  );

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">{collectionName}</h1>
          <p className="text-sm text-muted-foreground">
            Manage your {collectionName.toLowerCase()} content
          </p>
        </div>
        <Link
          to="/sites/$siteId/content/$collectionSlug/new"
          params={{ siteId, collectionSlug }}
          className={buttonVariants()}
        >
          <Plus data-icon="inline-start" />
          New {collectionName}
        </Link>
      </div>

      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="Search..."
            value={search}
            onChange={(e) => handleSearchChange(e.target.value)}
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
          onValueChange={handleStatusChange}
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

      <DataTable
        columns={columns}
        data={items}
        total={total}
        page={page}
        pageSize={pageSize}
        onPageChange={setPage}
        onPageSizeChange={(size) => {
          setPageSize(size);
          setPage(1);
        }}
        isLoading={isLoading}
        emptyMessage="No content yet"
        emptyDescription={`Create your first ${collectionName.toLowerCase()} to get started.`}
      />
    </div>
  );
}
