import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { DynamicForm } from "@/components/dynamic-form";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getCollection,
  getContentById,
  type SchemaDefinition,
  updateContent,
} from "@/lib/api";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/content/$collectionSlug/$id/edit",
)({
  component: EditContentPage,
});

function EditContentPage() {
  const { siteId, collectionSlug, id } = Route.useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [data, setData] = useState<Record<string, unknown>>({});
  const [slug, setSlug] = useState("");
  const [initialized, setInitialized] = useState(false);

  const { data: collection, isLoading: collectionLoading } = useQuery({
    queryKey: ["collection", siteId, collectionSlug],
    queryFn: () => getCollection(siteId, collectionSlug),
  });

  const { data: content, isLoading: contentLoading } = useQuery({
    queryKey: ["content", siteId, id],
    queryFn: () => getContentById(siteId, id),
    enabled: !!id,
  });

  useEffect(() => {
    if (content && !initialized) {
      const parsedData =
        typeof content.data === "string"
          ? JSON.parse(content.data)
          : content.data;
      setData(parsedData as Record<string, unknown>);
      setSlug(content.slug);
      setInitialized(true);
    }
  }, [content, initialized]);

  const updateMutation = useMutation({
    mutationFn: () =>
      updateContent(siteId, id, {
        data,
        slug,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["content", siteId, id],
      });
      toast.success("Content updated");
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

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!slug.trim()) return;
    updateMutation.mutate();
  };

  const isLoading = collectionLoading || contentLoading;

  if (isLoading || !initialized) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!collection || !content) {
    return (
      <div className="p-6">
        <p>Content not found.</p>
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
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-semibold">Edit {collection.name}</h1>
            <Badge
              variant={content.status === "published" ? "default" : "secondary"}
            >
              {content.status}
            </Badge>
          </div>
          <p className="text-sm text-muted-foreground">
            Edit {collection.name.toLowerCase()} #{content.id.slice(0, 8)}
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
                onChange={setData}
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
              <label htmlFor="edit-slug" className="text-sm font-medium">
                Slug
              </label>
              <Input
                id="edit-slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
              />
            </div>
          </CardContent>
        </Card>

        <div className="flex gap-2">
          <Button
            type="submit"
            disabled={updateMutation.isPending || !slug.trim()}
          >
            {updateMutation.isPending ? "Saving..." : "Save"}
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
