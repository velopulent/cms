import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { DynamicForm } from "@/components/dynamic-form";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getCollection,
  getContentById,
  type SchemaDefinition,
  updateContent,
} from "@/lib/api";

function buildDataSchema(schema: SchemaDefinition) {
  const shape: Record<string, z.ZodTypeAny> = {};
  for (const f of schema.fields) {
    switch (f.type) {
      case "number":
        shape[f.name] = f.required
          ? z.number({ error: `${f.name} is required` })
          : z.number().optional();
        break;
      case "boolean":
        shape[f.name] = z.boolean().optional();
        break;
      default:
        shape[f.name] = f.required
          ? z.string().min(1, `${f.name.replace(/_/g, " ")} is required`)
          : z.string().optional();
    }
  }
  return z.object(shape);
}

export const Route = createFileRoute(
  "/_admin/sites/$siteId/content/$collectionSlug/$id/edit",
)({
  component: EditContentPage,
});

function EditContentPage() {
  const { siteId, collectionSlug, id } = Route.useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
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

  let collectionDef: SchemaDefinition | null = null;
  if (collection) {
    try {
      collectionDef = JSON.parse(collection.definition);
    } catch {
      // invalid collection
    }
  }

  const contentSchema = z.object({
    data: collectionDef ? buildDataSchema(collectionDef) : z.object({}),
    slug: z.string().min(1, "Slug is required"),
  });

  const form = useForm({
    defaultValues: {
      data: {} as Record<string, unknown>,
      slug: "",
    },
    validators: {
      onSubmit: contentSchema,
    },
    onSubmit: async ({ value }) => {
      updateMutation.mutate({
        data: value.data,
        slug: value.slug,
      });
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({
      data,
      slug,
    }: {
      data: Record<string, unknown>;
      slug: string;
    }) => updateContent(siteId, id, { data, slug }),
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

  useEffect(() => {
    if (content && !initialized) {
      const parsedData =
        typeof content.data === "string"
          ? JSON.parse(content.data)
          : content.data;
      form.setFieldValue("data", parsedData as Record<string, unknown>);
      form.setFieldValue("slug", content.slug);
      setInitialized(true);
    }
  }, [content, initialized, form]);

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

      <form
        onSubmit={(e) => {
          e.preventDefault();
          form.handleSubmit();
        }}
        className="flex flex-col gap-6"
      >
        <Card>
          <CardHeader>
            <CardTitle>Content</CardTitle>
          </CardHeader>
          <CardContent>
            {collectionDef ? (
              <DynamicForm
                fields={collectionDef.fields}
                form={form}
                prefix="data"
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
            <FieldGroup>
              <form.Field
                name="slug"
                children={(field) => {
                  const isInvalid =
                    field.state.meta.isTouched && !field.state.meta.isValid;
                  return (
                    <Field data-invalid={isInvalid}>
                      <FieldLabel htmlFor={field.name}>Slug</FieldLabel>
                      <Input
                        id={field.name}
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => field.handleChange(e.target.value)}
                        aria-invalid={isInvalid}
                      />
                      {isInvalid && (
                        <FieldError errors={field.state.meta.errors} />
                      )}
                    </Field>
                  );
                }}
              />
            </FieldGroup>
          </CardContent>
        </Card>

        <div className="flex gap-2">
          <Button type="submit" disabled={updateMutation.isPending}>
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
