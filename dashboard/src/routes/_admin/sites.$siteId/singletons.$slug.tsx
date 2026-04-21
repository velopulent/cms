import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { DynamicForm } from "@/components/dynamic-form";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getSingleton,
  type SchemaDefinition,
  updateSingletonData,
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

export const Route = createFileRoute("/_admin/sites/$siteId/singletons/$slug")({
  component: SingletonEditPage,
});

function SingletonEditPage() {
  const { siteId, slug } = Route.useParams();
  const queryClient = useQueryClient();
  const [initialized, setInitialized] = useState(false);

  const { data: singleton, isLoading } = useQuery({
    queryKey: ["singleton", siteId, slug],
    queryFn: () => getSingleton(siteId, slug),
  });

  let definition: SchemaDefinition | null = null;
  if (singleton?.definition) {
    if (typeof singleton.definition === "string") {
      try {
        definition = JSON.parse(singleton.definition);
      } catch {
        // invalid
      }
    } else {
      definition = singleton.definition as SchemaDefinition;
    }
  }

  const dataSchema = z.object({
    data: definition ? buildDataSchema(definition) : z.object({}),
  });

  const form = useForm({
    defaultValues: {
      data: {} as Record<string, unknown>,
    },
    validators: {
      onSubmit: dataSchema,
    },
    onSubmit: async ({ value }) => {
      saveMutation.mutate(value.data);
    },
  });

  const saveMutation = useMutation({
    mutationFn: (data: Record<string, unknown>) =>
      updateSingletonData(siteId, slug, data),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["singleton", siteId, slug],
      });
      toast.success("Singleton updated");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  useEffect(() => {
    if (singleton && !initialized) {
      if (singleton.data) {
        form.setFieldValue("data", singleton.data);
      }
      setInitialized(true);
    }
  }, [singleton, initialized, form]);

  if (isLoading || !initialized) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!singleton) {
    return (
      <div className="p-6">
        <p>Singleton not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div className="flex items-center gap-3">
        <Link
          to="/sites/$siteId/collections"
          params={{ siteId }}
          className={buttonVariants({ variant: "ghost", size: "icon" })}
        >
          <ArrowLeft />
        </Link>
        <div>
          <h1 className="text-2xl font-semibold">{singleton.name}</h1>
          <p className="text-sm text-muted-foreground">Singleton · {slug}</p>
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
            {definition ? (
              <DynamicForm
                fields={definition.fields}
                form={form}
                prefix="data"
                siteId={siteId}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Invalid singleton definition.
              </p>
            )}
          </CardContent>
        </Card>

        <div className="flex gap-2">
          <Button type="submit" disabled={saveMutation.isPending}>
            {saveMutation.isPending ? "Saving..." : "Save"}
          </Button>
          <Link
            to="/sites/$siteId/collections"
            params={{ siteId }}
            className={buttonVariants({ variant: "outline" })}
          >
            Back
          </Link>
        </div>
      </form>
    </div>
  );
}
