import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, History } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { DynamicForm } from "@/components/dynamic-form";
import { RevisionsPanel } from "@/components/revisions-panel";
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
  getEntryById,
  type SchemaDefinition,
  updateEntry,
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
  "/_admin/sites/$siteId/entries/$collectionSlug/$id/edit",
)({
  component: EditEntryPage,
});

function EditEntryPage() {
  const { siteId, collectionSlug, id } = Route.useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [hasLoadedOnce, setHasLoadedOnce] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [changeSummary, setChangeSummary] = useState("");
  const lastHydratedRef = useRef<string | null>(null);

  const { data: collection, isLoading: collectionLoading } = useQuery({
    queryKey: ["collection", siteId, collectionSlug],
    queryFn: () => getCollection(siteId, collectionSlug),
  });

  const { data: entry, isLoading: entryLoading } = useQuery({
    queryKey: ["entry", siteId, id],
    queryFn: () => getEntryById(siteId, id),
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

  const entrySchema = z.object({
    data: collectionDef ? buildDataSchema(collectionDef) : z.object({}),
    slug: z.string().min(1, "Slug is required"),
  });

  const form = useForm({
    defaultValues: {
      data: {} as Record<string, unknown>,
      slug: "",
    },
    validators: {
      onSubmit: entrySchema,
    },
    onSubmit: async ({ value }) => {
      updateMutation.mutate({
        data: value.data,
        slug: value.slug,
        change_summary: changeSummary.trim() || undefined,
      });
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({
      data,
      slug,
      change_summary,
    }: {
      data: Record<string, unknown>;
      slug: string;
      change_summary?: string;
    }) => updateEntry(siteId, id, { data, slug, change_summary }),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["entry", siteId, id],
      });
      toast.success("Entry updated");
      setChangeSummary("");
      navigate({
        to: "/sites/$siteId/entries/$collectionSlug",
        params: { siteId, collectionSlug },
      });
    },
    onError: (err: Error) => toast.error(err.message),
  });

  useEffect(() => {
    if (entry && entry.updated_at !== lastHydratedRef.current) {
      const parsedData =
        typeof entry.data === "string" ? JSON.parse(entry.data) : entry.data;
      form.setFieldValue("data", parsedData as Record<string, unknown>);
      form.setFieldValue("slug", entry.slug);
      lastHydratedRef.current = entry.updated_at;
    }
  }, [entry, form]);

  useEffect(() => {
    if (entry && !hasLoadedOnce) {
      setHasLoadedOnce(true);
    }
  }, [entry, hasLoadedOnce]);

  const isLoading = collectionLoading || entryLoading;

  if (isLoading || !hasLoadedOnce) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!collection || !entry) {
    return (
      <div className="p-6">
        <p>Entry not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Link
          to="/sites/$siteId/entries/$collectionSlug"
          params={{ siteId, collectionSlug }}
          className={buttonVariants({ variant: "ghost", size: "icon" })}
        >
          <ArrowLeft />
        </Link>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap justify-between">
            <div className="flex items-center gap-2">
              <h1 className="text-2xl font-semibold">Edit {collection.name}</h1>
              <Badge
                variant={entry.status === "published" ? "default" : "secondary"}
              >
                {entry.status}
              </Badge>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setHistoryOpen(true)}
              className="ml-auto sm:ml-0"
            >
              <History data-icon="inline-start" />
              History
            </Button>
          </div>
          <p className="text-sm text-muted-foreground">{entry.slug}</p>
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
            <CardTitle>Entry</CardTitle>
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

        <FieldGroup className="max-w-lg">
          <Field>
            <FieldLabel>Change summary (optional)</FieldLabel>
            <Input
              placeholder="What changed in this update?"
              value={changeSummary}
              onChange={(e) => setChangeSummary(e.target.value)}
            />
          </Field>
        </FieldGroup>

        <div className="flex gap-2">
          <Button type="submit" disabled={updateMutation.isPending}>
            {updateMutation.isPending ? "Saving..." : "Save"}
          </Button>
          <Link
            to="/sites/$siteId/entries/$collectionSlug"
            params={{ siteId, collectionSlug }}
            className={buttonVariants({ variant: "outline" })}
          >
            Cancel
          </Link>
        </div>
      </form>

      <RevisionsPanel
        entryId={id}
        siteId={siteId}
        open={historyOpen}
        onOpenChange={setHistoryOpen}
        collectionDef={collectionDef}
        onRestore={(restored) => {
          queryClient.setQueryData(["entry", siteId, id], restored);
        }}
      />
    </div>
  );
}
