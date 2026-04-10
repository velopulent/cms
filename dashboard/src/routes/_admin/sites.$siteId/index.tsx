import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { FileText, Pencil, Plus, Square } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  type Collection,
  type Entry,
  getCollections,
  getEntries,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/")({
  component: DashboardPage,
});

function DashboardPage() {
  const { siteId } = Route.useParams();

  const { data: collections, isLoading: collectionsLoading } = useQuery({
    queryKey: ["collections", siteId],
    queryFn: () => getCollections(siteId),
  });

  const { data: entriesResponse, isLoading: entriesLoading } = useQuery({
    queryKey: ["entries", siteId, "all"],
    queryFn: () => getEntries(siteId, {}),
  });

  const collectionsArray = Array.isArray(collections) ? collections : [];
  const regularCollections = collectionsArray.filter(
    (c) => !c.is_singleton,
  );
  const singletons = collectionsArray.filter((c) => c.is_singleton);

  const allEntriesArray = entriesResponse?.items ?? [];
  const publishedCount = allEntriesArray.filter(
    (e: Entry) => e.status === "published",
  ).length;
  const draftCount = allEntriesArray.filter(
    (e: Entry) => e.status === "draft",
  ).length;

  return (
    <div className="flex flex-col gap-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold">Dashboard</h1>
        <p className="text-sm text-muted-foreground">
          Overview of your content
        </p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Collections
            </CardTitle>
          </CardHeader>
          <CardContent>
            {collectionsLoading ? (
              <Skeleton className="h-8 w-12" />
            ) : (
              <p className="text-3xl font-semibold">
                {regularCollections.length}
              </p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Singletons
            </CardTitle>
          </CardHeader>
          <CardContent>
            {collectionsLoading ? (
              <Skeleton className="h-8 w-12" />
            ) : (
              <p className="text-3xl font-semibold">{singletons.length}</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Published
            </CardTitle>
          </CardHeader>
          <CardContent>
            {entriesLoading ? (
              <Skeleton className="h-8 w-12" />
            ) : (
              <p className="text-3xl font-semibold">{publishedCount}</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Drafts
            </CardTitle>
          </CardHeader>
          <CardContent>
            {entriesLoading ? (
              <Skeleton className="h-8 w-12" />
            ) : (
              <p className="text-3xl font-semibold">{draftCount}</p>
            )}
          </CardContent>
        </Card>
      </div>

      {singletons.length > 0 && (
        <div className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold">Singletons</h2>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {singletons.map((s: Collection) => (
              <Card key={s.id}>
                <CardContent className="flex items-center justify-between p-4">
                  <div className="flex items-center gap-3">
                    <Square className="size-4 text-muted-foreground" />
                    <p className="text-sm font-medium">{s.name}</p>
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
        </div>
      )}

      {regularCollections.length > 0 && (
        <div className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold">Quick Create</h2>
          <div className="flex flex-wrap gap-2">
            {regularCollections.map((c: Collection) => (
              <Link
                key={c.id}
                to="/sites/$siteId/entries/$collectionSlug/new"
                params={{ siteId, collectionSlug: c.slug }}
                className={buttonVariants({ variant: "outline" })}
              >
                <Plus data-icon="inline-start" />
                New {c.name}
              </Link>
            ))}
          </div>
        </div>
      )}

      {allEntriesArray.length > 0 && (
        <div className="flex flex-col gap-4">
          <h2 className="text-lg font-semibold">Recently Updated</h2>
          <div className="flex flex-col gap-2">
            {allEntriesArray.slice(0, 5).map((item: Entry) => {
              const collectionName = collections?.find(
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
                  className="flex items-center justify-between rounded-lg border p-3"
                >
                  <div className="flex items-center gap-3">
                    <FileText className="size-4 text-muted-foreground" />
                    <div>
                      <p className="text-sm font-medium">{title}</p>
                      <p className="text-xs text-muted-foreground">
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
        </div>
      )}
    </div>
  );
}
