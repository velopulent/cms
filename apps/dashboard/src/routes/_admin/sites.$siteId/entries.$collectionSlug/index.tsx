import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { createColumns } from "@/components/entries/columns";
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
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import {
  deleteEntry,
  getCollection,
  getEntries,
  publishEntry,
  unpublishEntry,
} from "@/lib/api";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/entries/$collectionSlug/",
)({
  component: EntriesListPage,
});

function EntriesListPage() {
  const { siteId, collectionSlug } = Route.useParams();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search, 300);
  const [statusFilter, setStatusFilter] = useState<string>("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  // Reset to the first page when the debounced search term actually changes,
  // so paging tracks the fetched results rather than each keystroke.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset only on term change
  useEffect(() => {
    setPage(1);
  }, [debouncedSearch]);

  const { data: collection, isLoading: collectionLoading } = useQuery({
    queryKey: ["collection", siteId, collectionSlug],
    queryFn: () => getCollection(siteId, collectionSlug),
  });

  const { data: entriesResponse, isLoading: itemsLoading } = useQuery({
    queryKey: [
      "entries",
      siteId,
      collectionSlug,
      statusFilter,
      debouncedSearch,
      page,
      pageSize,
    ],
    queryFn: () =>
      getEntries(siteId, {
        type: collectionSlug,
        status: statusFilter || undefined,
        search: debouncedSearch || undefined,
        page,
        pageSize,
      }),
  });

  const items = entriesResponse?.items ?? [];
  const total = entriesResponse?.total ?? 0;

  const handleSearchChange = (value: string | null) => {
    setSearch(value || "");
  };

  const handleStatusChange = (value: string | null) => {
    setStatusFilter(value || "");
    setPage(1);
  };

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteEntry(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["entries", siteId],
      });
      toast.success("Entry deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const publishMutation = useMutation({
    mutationFn: (id: string) => publishEntry(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["entries", siteId],
      });
      toast.success("Published");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const unpublishMutation = useMutation({
    mutationFn: (id: string) => unpublishEntry(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["entries", siteId],
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
            Manage your {collectionName.toLowerCase()} entries
          </p>
        </div>
        <Link
          to="/sites/$siteId/entries/$collectionSlug/new"
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
            { label: "All statuses", value: "" },
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
        emptyMessage="No entries yet"
        emptyDescription={`Create your first ${collectionName.toLowerCase()} to get started.`}
      />
    </div>
  );
}
