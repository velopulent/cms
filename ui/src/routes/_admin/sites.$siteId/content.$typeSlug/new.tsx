import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { DynamicForm } from "@/components/dynamic-form";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  type ContentTypeSchema,
  createContent,
  getContentType,
} from "@/lib/api";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/content/$typeSlug/new",
)({
  component: CreateContentPage,
});

function CreateContentPage() {
  const { siteId, typeSlug } = Route.useParams();
  const navigate = useNavigate();
  const [data, setData] = useState<Record<string, unknown>>({});
  const [slug, setSlug] = useState("");
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(false);

  const { data: contentType, isLoading } = useQuery({
    queryKey: ["content-type", siteId, typeSlug],
    queryFn: () => getContentType(siteId, typeSlug),
  });

  const createMutation = useMutation({
    mutationFn: () =>
      createContent(siteId, {
        type_id: contentType!.id,
        data,
        slug,
      }),
    onSuccess: () => {
      toast.success("Content created");
      navigate({
        to: "/sites/$siteId/content/$typeSlug",
        params: { siteId, typeSlug },
      });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  let schema: ContentTypeSchema | null = null;
  if (contentType) {
    try {
      schema = JSON.parse(contentType.schema_json);
    } catch {
      // invalid schema
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
    if (!slug.trim() || !contentType) return;
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

  if (!contentType) {
    return (
      <div className="p-6">
        <p>Content type not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="icon"
          render={
            <Link
              to="/sites/$siteId/content/$typeSlug"
              params={{ siteId, typeSlug }}
            />
          }
        >
          <ArrowLeft />
        </Button>
        <div>
          <h1 className="text-2xl font-semibold">New {contentType.name}</h1>
          <p className="text-sm text-muted-foreground">
            Create a new {contentType.name.toLowerCase()}
          </p>
        </div>
      </div>

      <form onSubmit={handleSubmit} className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Content</CardTitle>
          </CardHeader>
          <CardContent>
            {schema ? (
              <DynamicForm
                fields={schema.fields}
                values={data}
                onChange={handleDataChange}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Invalid content type schema.
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
          <Button
            type="button"
            variant="outline"
            render={
              <Link
                to="/sites/$siteId/content/$typeSlug"
                params={{ siteId, typeSlug }}
              />
            }
          >
            Cancel
          </Button>
        </div>
      </form>
    </div>
  );
}
