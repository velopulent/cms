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
import {
  type SchemaDefinition,
  createContent,
  getSchema,
} from "@/lib/api";

export const Route = createFileRoute(
  "/_admin/sites/$siteId/content/$schemaSlug/new",
)({
  component: CreateContentPage,
});

function CreateContentPage() {
  const { siteId, schemaSlug } = Route.useParams();
  const navigate = useNavigate();
  const [data, setData] = useState<Record<string, unknown>>({});
  const [slug, setSlug] = useState("");
  const [slugManuallyEdited, setSlugManuallyEdited] = useState(false);

  const { data: schema, isLoading } = useQuery({
    queryKey: ["schema", siteId, schemaSlug],
    queryFn: () => getSchema(siteId, schemaSlug),
  });

  const createMutation = useMutation({
    mutationFn: () =>
      createContent(siteId, {
        schema_id: schema!.id,
        data,
        slug,
      }),
    onSuccess: () => {
      toast.success("Content created");
      navigate({
        to: "/sites/$siteId/content/$schemaSlug",
        params: { siteId, schemaSlug },
      });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  let schemaDef: SchemaDefinition | null = null;
  if (schema) {
    try {
      schemaDef = JSON.parse(schema.definition);
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
    if (!slug.trim() || !schema) return;
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

  if (!schema) {
    return (
      <div className="p-6">
        <p>Schema not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Link
          to="/sites/$siteId/content/$schemaSlug"
          params={{ siteId, schemaSlug }}
          className={buttonVariants({ variant: "ghost", size: "icon" })}
        >
          <ArrowLeft />
        </Link>
        <div>
          <h1 className="text-2xl font-semibold">New {schema.name}</h1>
          <p className="text-sm text-muted-foreground">
            Create a new {schema.name.toLowerCase()}
          </p>
        </div>
      </div>

      <form onSubmit={handleSubmit} className="flex flex-col gap-6">
        <Card>
          <CardHeader>
            <CardTitle>Content</CardTitle>
          </CardHeader>
          <CardContent>
            {schemaDef ? (
              <DynamicForm
                fields={schemaDef.fields}
                values={data}
                onChange={handleDataChange}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Invalid schema definition.
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
            to="/sites/$siteId/content/$schemaSlug"
            params={{ siteId, schemaSlug }}
            className={buttonVariants({ variant: "outline" })}
          >
            Cancel
          </Link>
        </div>
      </form>
    </div>
  );
}
