import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { DynamicForm } from "@/components/dynamic-form";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { createContent, getCollection, type SchemaDefinition } from "@/lib/api";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/content/$collectionSlug/new",
)({
  component: CreateContentPage,
});

function CreateContentPage() {
  const { siteId, collectionSlug } = Route.useParams();
  const navigate = useNavigate();
  const [data, setData] = useState<Record<string, unknown>>({});
  const [slug, setSlug] = useState("");
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(false);

  const { data: collection, isLoading } = useQuery({
    queryKey: ["collection", siteId, collectionSlug],
    queryFn: () => getCollection(siteId, collectionSlug),
  });

  const createMutation = useMutation({
    mutationFn: () =>
      createContent(siteId, {
        collection_id: collection!.id,
        data,
        slug,
      }),
    onSuccess: () => {
      toast.success("Content created");
      navigate({
        to: "/sites/$siteId/content/$collectionSlug",
        params: { siteId, collectionSlug },
      });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  let collectionDef: SchemaDefinition | null = null;
  if (collection) {
    try {
      collectionDef = JSON.parse(collection.definition);
    } catch {
      // invalid collection
    }
  }

  const handleDataChange = (newData: Record<string, unknown>) => {
    setData(newData);
    if (!slugManuallyEdited) {
      const titleField = newData.title ?? newData.name ?? "";
      if (typeof titleField === "string" && titleField) {
        setSlug(
          titleField
            .toLowerCase()
            .trim()
            .replace(/[^\w\s-]/g, "")
            .replace(/[\s_]+/g, "-")
            .replace(/-+/g, "-"),
        );
      }
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!slug.trim() || !collection) return;
    createMutation.mutate();
  };

  if (isLoading) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!collection) {
    return (
      <div className="p-6">
        <p>Collection not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Link
          to="/sites/$siteId/content/$collectionSlug"
          params={{ siteId, collectionSlug }}
          className={buttonVariants({ variant: "ghost", size: "icon" })}
        >
          <ArrowLeft />
        </Link>
        <div>
          <h1 className="text-2xl font-semibold">New {collection.name}</h1>
          <p className="text-sm text-muted-foreground">
            Create a new {collection.name.toLowerCase()}
          </p>
        </div>
      </div>

      <form onSubmit={handleSubmit} className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Content</CardTitle>
          </CardHeader>
          <CardContent>
            {collectionDef ? (
              <DynamicForm
                fields={collectionDef.fields}
                values={data}
                onChange={handleDataChange}
                siteId={siteId}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Invalid collection definition.
              </p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Settings</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col gap-2">
              <label htmlFor="content-slug" className="text-sm font-medium">
                Slug
              </label>
              <Input
                id="content-slug"
                placeholder="my-content-slug"
                value={slug}
                onChange={(e) => {
                  setSlug(e.target.value);
                  setSlugManuallyEdited(true);
                }}
              />
            </div>
          </CardContent>
        </Card>

        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={createMutation.isPending || !slug.trim()}
          >
            {createMutation.isPending ? "Creating..." : "Create"}
          </Button>
          <Link
            to="/sites/$siteId/content/$collectionSlug"
            params={{ siteId, collectionSlug }}
            className={buttonVariants({ variant: "outline" })}
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  );
}
