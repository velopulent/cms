import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { AlertTriangle, Cloud, HardDrive } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  type ApiKey,
  type ApiKeyResponse,
  createApiKey,
  deleteApiKey,
  getApiKeys,
  getSite,
  updateSite,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/settings")({
  component: SiteSettingsPage,
});

const siteSettingsSchema = z.object({
  name: z.string().min(1, "Site name is required"),
  storageProvider: z.string(),
});

const apiKeySchema = z.object({
  name: z.string().min(1, "Key name is required"),
});

function SiteSettingsPage() {
  const { siteId } = Route.useParams();
  const queryClient = useQueryClient();
  const [initialized, setInitialized] = useState(false);

  const { data: site, isLoading } = useQuery({
    queryKey: ["site", siteId],
    queryFn: () => getSite(siteId),
  });

  const form = useForm({
    defaultValues: {
      name: "",
      storageProvider: "",
    },
    validators: {
      onSubmit: siteSettingsSchema,
    },
    onSubmit: async ({ value }) => {
      updateMutation.mutate(value);
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({
      name,
      storageProvider,
    }: {
      name: string;
      storageProvider: string;
    }) =>
      updateSite(siteId, {
        name,
        default_storage_provider: storageProvider,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["site", siteId] });
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Site settings updated");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  useEffect(() => {
    if (site && !initialized) {
      form.reset();
      form.setFieldValue("name", site.name);
      form.setFieldValue("storageProvider", site.default_storage_provider);
      setInitialized(true);
    }
  }, [site, initialized, form]);

  if (isLoading || !initialized) {
    return (
      <div className="flex flex-col gap-6 p-6">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-64 w-full max-w-lg" />
      </div>
    );
  }

  if (!site) {
    return (
      <div className="p-6">
        <p>Site not found.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold">Settings</h1>
        <p className="text-sm text-muted-foreground">
          Manage your site settings
        </p>
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
            <CardTitle>General</CardTitle>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <form.Field
                name="name"
                children={(field) => {
                  const isInvalid =
                    field.state.meta.isTouched && !field.state.meta.isValid;
                  return (
                    <Field data-invalid={isInvalid}>
                      <FieldLabel htmlFor={field.name}>Site Name</FieldLabel>
                      <Input
                        id={field.name}
                        name={field.name}
                        placeholder="My Site"
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => field.handleChange(e.target.value)}
                        className="max-w-md"
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

        <Card>
          <CardHeader>
            <CardTitle>File Storage</CardTitle>
            <CardDescription>
              Choose where uploaded files will be stored
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            <FieldGroup>
              <form.Field
                name="storageProvider"
                children={(field) => {
                  return (
                    <Field>
                      <FieldLabel htmlFor={field.name}>
                        Storage Provider
                      </FieldLabel>
                      <Select
                        value={field.state.value}
                        onValueChange={(v) => v && field.handleChange(v)}
                      >
                        <SelectTrigger
                          id={field.name}
                          className="w-full max-w-md"
                          aria-invalid={
                            field.state.meta.isTouched &&
                            !field.state.meta.isValid
                          }
                        >
                          {field.state.value === "filesystem" ? (
                            <div className="flex items-center gap-2">
                              <HardDrive className="size-4" />
                              <span>Filesystem</span>
                            </div>
                          ) : field.state.value === "s3" ? (
                            <div className="flex items-center gap-2">
                              <Cloud className="size-4" />
                              <span>S3 / Cloud Storage</span>
                            </div>
                          ) : (
                            <SelectValue placeholder="Select storage type" />
                          )}
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="filesystem">
                            <div className="flex items-center gap-2">
                              <HardDrive className="size-4" />
                              <span>Filesystem</span>
                            </div>
                          </SelectItem>
                          <SelectItem value="s3">
                            <div className="flex items-center gap-2">
                              <Cloud className="size-4" />
                              <span>S3 / Cloud Storage</span>
                            </div>
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </Field>
                  );
                }}
              />
            </FieldGroup>

            {form.getFieldValue("name") !== site?.name ||
            form.getFieldValue("storageProvider") !==
              site?.default_storage_provider ? (
              <div className="flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 p-3">
                <AlertTriangle className="mt-0.5 size-4 text-amber-600" />
                <div className="text-sm text-amber-800">
                  <p className="font-medium">
                    Changing storage will only affect new uploads.
                  </p>
                  <p className="text-amber-700">
                    Existing files will stay where they are. Make sure your S3
                    bucket is properly configured before switching.
                  </p>
                </div>
              </div>
            ) : null}
          </CardContent>
        </Card>

        <Button
          type="submit"
          className="w-fit"
          disabled={updateMutation.isPending}
        >
          {updateMutation.isPending ? "Saving..." : "Save Changes"}
        </Button>
      </form>

      <ApiKeysSection siteId={siteId} />
    </div>
  );
}

function ApiKeysSection({ siteId }: { siteId: string }) {
  const queryClient = useQueryClient();
  const [createdKey, setCreatedKey] = useState<ApiKeyResponse | null>(null);

  const { data: apiKeys, isLoading } = useQuery({
    queryKey: ["api-keys", siteId],
    queryFn: () => getApiKeys(siteId),
  });

  const createMutation = useMutation({
    mutationFn: ({ name }: { name: string }) => createApiKey(siteId, name),
    onSuccess: (key) => {
      queryClient.invalidateQueries({ queryKey: ["api-keys", siteId] });
      setCreatedKey(key);
      apiKeyForm.reset();
      toast.success("API key created");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (keyId: string) => deleteApiKey(siteId, keyId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["api-keys", siteId] });
      toast.success("API key deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const apiKeyForm = useForm({
    defaultValues: {
      name: "",
    },
    validators: {
      onSubmit: apiKeySchema,
    },
    onSubmit: async ({ value }) => {
      createMutation.mutate(value);
    },
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>API Keys</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <p className="text-sm text-muted-foreground">
          API keys allow external applications to read published content from
          this site. Use the API reference at{" "}
          <a
            href="/api/v1/docs"
            target="_blank"
            rel="noopener noreferrer"
            className="underline"
          >
            /api/v1/docs
          </a>{" "}
          to explore the API.
        </p>

        {createdKey && (
          <div className="rounded-md border border-yellow-500 bg-yellow-50 p-4">
            <p className="text-sm font-medium text-yellow-800">
              Copy this key now — it won't be shown again.
            </p>
            <code className="mt-2 block break-all rounded bg-yellow-100 p-2 text-sm">
              {createdKey.key}
            </code>
            <Button
              variant="outline"
              size="sm"
              className="mt-2"
              onClick={() => {
                navigator.clipboard.writeText(createdKey.key);
                toast.success("Copied to clipboard");
              }}
            >
              Copy
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="mt-2 ml-2"
              onClick={() => setCreatedKey(null)}
            >
              Dismiss
            </Button>
          </div>
        )}

        <form
          onSubmit={(e) => {
            e.preventDefault();
            apiKeyForm.handleSubmit();
          }}
          className="flex gap-2"
        >
          <apiKeyForm.Field
            name="name"
            children={(field) => {
              const isInvalid =
                field.state.meta.isTouched && !field.state.meta.isValid;
              return (
                <Field data-invalid={isInvalid} className="flex-1">
                  <Input
                    id={field.name}
                    name={field.name}
                    placeholder="Key name (e.g., Production Website)"
                    value={field.state.value}
                    onBlur={field.handleBlur}
                    onChange={(e) => field.handleChange(e.target.value)}
                    className="max-w-sm"
                    aria-invalid={isInvalid}
                  />
                  {isInvalid && <FieldError errors={field.state.meta.errors} />}
                </Field>
              );
            }}
          />
          <Button type="submit" disabled={createMutation.isPending}>
            {createMutation.isPending ? "Creating..." : "Create Key"}
          </Button>
        </form>

        {isLoading ? (
          <Skeleton className="h-20 w-full" />
        ) : apiKeys && apiKeys.length > 0 ? (
          <div className="flex flex-col gap-2">
            {apiKeys.map((key: ApiKey) => (
              <div
                key={key.id}
                className="flex items-center justify-between rounded-md border p-3"
              >
                <div>
                  <p className="font-medium">{key.name}</p>
                  <p className="text-sm text-muted-foreground">
                    {key.key_prefix}... &middot; Created{" "}
                    {new Date(key.created_at).toLocaleDateString()}
                    {key.last_used_at && (
                      <>
                        {" "}
                        &middot; Last used{" "}
                        {new Date(key.last_used_at).toLocaleDateString()}
                      </>
                    )}
                  </p>
                </div>
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() => deleteMutation.mutate(key.id)}
                  disabled={deleteMutation.isPending}
                >
                  Delete
                </Button>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            No API keys yet. Create one to get started.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
