import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
  ArrowRight,
  FileText,
  Layers,
  Pencil,
  Plus,
  Square,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  type Collection,
  type Entry,
  getCollections,
  getEntries,
  getSite,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/")({
  component: DashboardPage,
});

function StatPill({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex flex-col rounded-lg border bg-card px-4 py-3">
      <span className="text-2xl font-semibold tabular-nums">{value}</span>
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  );
}

function DashboardPage() {
  const { siteId } = Route.useParams();

  const { data: site } = useQuery({
    queryKey: ["site", siteId],
    queryFn: () => getSite(siteId),
  });

  const { data: collections, isLoading: collectionsLoading } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId),
  });

  const { data: entriesResponse, isLoading: entriesLoading } = useQuery({
    queryKey: ["entries", siteId, "all"],
    queryFn: () => getEntries(siteId, {}),
  });

  const collectionsArray = Array.isArray(collections) ? collections : [];
  const regularCollections = collectionsArray.filter((c) => !c.is_singleton);
  const singletons = collectionsArray.filter((c) => c.is_singleton);

  const allEntriesArray = entriesResponse?.items ?? [];
  const publishedCount = allEntriesArray.filter(
    (e: Entry) => e.status === "published",
  ).length;
  const draftCount = allEntriesArray.filter(
    (e: Entry) => e.status === "draft",
  ).length;

  const countFor = (collectionId: string) =>
    allEntriesArray.filter((e: Entry) => e.collection_id === collectionId)
      .length;

  return (
    <div className="flex flex-col gap-8 p-4 sm:p-6">
      {/* Hero */}
      <div className="flex flex-col gap-4 rounded-xl border bg-gradient-to-br from-muted/60 to-background p-6">
        <div>
          <p className="text-sm text-muted-foreground">Dashboard</p>
          <h1 className="text-2xl font-semibold tracking-tight">
            {site?.name ?? <Skeleton className="inline-block h-7 w-40" />}
          </h1>
        </div>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          {collectionsLoading || entriesLoading ? (
            [0, 1, 2, 3].map((i) => (
              <Skeleton key={i} className="h-16 w-full" />
            ))
          ) : (
            <>
              <StatPill label="Collections" value={regularCollections.length} />
              <StatPill label="Singletons" value={singletons.length} />
              <StatPill label="Published" value={publishedCount} />
              <StatPill label="Drafts" value={draftCount} />
            </>
          )}
        </div>
      </div>

      {/* Collections */}
      <section className="flex flex-col gap-4">
        <h2 className="text-lg font-semibold">Collections</h2>
        {collectionsLoading ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {[0, 1, 2].map((i) => (
              <Skeleton key={i} className="h-36 w-full" />
            ))}
          </div>
        ) : regularCollections.length > 0 ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {regularCollections.map((c: Collection) => (
              <Card key={c.id} className="flex flex-col">
                <CardHeader className="pb-2">
                  <div className="flex items-center gap-2">
                    <div className="flex size-8 items-center justify-center rounded-md bg-primary/10">
                      <Layers className="size-4 text-primary" />
                    </div>
                    <CardTitle className="truncate text-base">
                      {c.name}
                    </CardTitle>
                  </div>
                </CardHeader>
                <CardContent className="mt-auto flex flex-col gap-3">
                  <p className="text-sm text-muted-foreground">
                    {countFor(c.id)}{" "}
                    {countFor(c.id) === 1 ? "entry" : "entries"}
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <Link
                      to="/sites/$siteId/entries/$collectionSlug/new"
                      params={{ siteId, collectionSlug: c.slug }}
                      className={buttonVariants({ size: "sm" })}
                    >
                      <Plus data-icon="inline-start" />
                      New entry
                    </Link>
                    <Link
                      to="/sites/$siteId/entries/$collectionSlug"
                      params={{ siteId, collectionSlug: c.slug }}
                      className={buttonVariants({
                        variant: "outline",
                        size: "sm",
                      })}
                    >
                      View
                      <ArrowRight data-icon="inline-end" />
                    </Link>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed py-12 text-center">
            <Layers className="size-8 text-muted-foreground" />
            <p className="text-sm text-muted-foreground">No collections yet.</p>
            <Link
              to="/sites/$siteId/collections"
              params={{ siteId }}
              className={buttonVariants({ size: "sm" })}
            >
              <Plus data-icon="inline-start" />
              Create collection
            </Link>
          </div>
        )}
      </section>

      {/* Singletons */}
      {singletons.length > 0 && (
        <section className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold">Singletons</h2>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {singletons.map((s: Collection) => (
              <Card key={s.id}>
                <CardContent className="flex items-center justify-between p-4">
                  <div className="flex min-w-0 items-center gap-3">
                    <Square className="size-4 shrink-0 text-muted-foreground" />
                    <p className="truncate text-sm font-medium">{s.name}</p>
                  </div>
                  <Link
                    to="/sites/$siteId/singletons/$slug"
                    params={{ siteId, slug: s.slug }}
                    className={buttonVariants({ variant: "ghost", size: "sm" })}
                  >
                    <Pencil className="size-4" />
                  </Link>
                </CardContent>
              </Card>
            ))}
          </div>
        </section>
      )}

      {/* Recently updated */}
      {allEntriesArray.length > 0 && (
        <section className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold">Recently updated</h2>
          <div className="flex flex-col gap-2">
            {allEntriesArray.slice(0, 5).map((item: Entry) => {
              const collectionName = collectionsArray.find(
                (c: Collection) => c.id === item.collection_id,
              )?.name;
              let title: string;
              try {
                const parsedData =
                  typeof item.data === "string"
                    ? JSON.parse(item.data)
                    : item.data;
                title =
                  (parsedData.title as string) ||
                  (parsedData.name as string) ||
                  item.slug;
              } catch {
                title = item.slug;
              }
              return (
                <div
                  key={item.id}
                  className="flex items-center justify-between gap-3 rounded-lg border p-3"
                >
                  <div className="flex min-w-0 items-center gap-3">
                    <FileText className="size-4 shrink-0 text-muted-foreground" />
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium">{title}</p>
                      <p className="truncate text-xs text-muted-foreground">
                        {collectionName} · {item.slug}
                      </p>
                    </div>
                  </div>
                  <Badge
                    variant={
                      item.status === "published" ? "default" : "secondary"
                    }
                  >
                    {item.status}
                  </Badge>
                </div>
              );
            })}
          </div>
        </section>
      )}
    </div>
  );
}
