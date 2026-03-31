import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { DynamicForm } from "@/components/dynamic-form";
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
import { createContent, getCollection, type SchemaDefinition } from "@/lib/api";

function buildDefaultValues(schema: SchemaDefinition) {
  const defaults: Record<string, unknown> = {};
  for (const f of schema.fields) {
    switch (f.type) {
      case "number":
        defaults[f.name] = 0;
        break;
      case "boolean":
        defaults[f.name] = false;
        break;
      default:
        defaults[f.name] = "";
    }
  }
  return defaults;
}

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
  "/_admin/sites/$siteId/content/$collectionSlug/new",
)({
  component: CreateContentPage,
});

function CreateContentPage() {
  const { siteId, collectionSlug } = Route.useParams();
  const navigate = useNavigate();
  const [schemaReady, setSchemaReady] = useState(false);

  const { data: collection, isLoading } = useQuery({
    queryKey: ["collection", siteId, collectionSlug],
    queryFn: () => getCollection(siteId, collectionSlug),
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
      if (!collection) return;
      createMutation.mutate({
        collection_id: collection.id,
        data: value.data,
        slug: value.slug,
      });
    },
  });

  const createMutation = useMutation({
    mutationFn: ({
      collection_id,
      data,
      slug,
    }: {
      collection_id: string;
      data: Record<string, unknown>;
      slug: string;
    }) => createContent(siteId, { collection_id, data, slug }),
    onSuccess: () => {
      toast.success("Content created");
      navigate({
        to: "/sites/$siteId/content/$collectionSlug",
        params: { siteId, collectionSlug },
      });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  useEffect(() => {
    if (collectionDef && !schemaReady) {
      const defaults = buildDefaultValues(collectionDef);
      form.setFieldValue("data", defaults);
      setSchemaReady(true);
    }
  }, [collectionDef, schemaReady, form]);

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
            {collectionDef && schemaReady ? (
              <DynamicForm
                fields={collectionDef.fields}
                form={form}
                prefix="data"
                siteId={siteId}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Loading collection schema...
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
                        placeholder="my-content-slug"
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => {
                          field.handleChange(e.target.value);
                        }}
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
          <Button type="submit" disabled={createMutation.isPending}>
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
