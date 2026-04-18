import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Copy, Key, Shield } from "lucide-react";
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
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
});

const apiKeySchema = z.object({
  name: z.string().min(1, "Key name is required"),
  permissions: z.enum(["read", "write"]),
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
    },
    validators: {
      onSubmit: siteSettingsSchema,
    },
    onSubmit: async ({ value }) => {
      updateMutation.mutate(value);
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ name }: { name: string }) =>
      updateSite(siteId, { name }),
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
    <div className="flex flex-col gap-6 p-6 w-full max-w-4xl mx-auto">
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
  const [copied, setCopied] = useState(false);

  const { data: apiKeys, isLoading } = useQuery({
    queryKey: ["api-keys", siteId],
    queryFn: () => getApiKeys(siteId),
  });

  const createMutation = useMutation({
    mutationFn: ({ name, permissions }: { name: string; permissions: string }) =>
      createApiKey(siteId, name, permissions),
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
      permissions: "read" as "read" | "write",
    },
    validators: {
      onSubmit: apiKeySchema,
    },
    onSubmit: async ({ value }) => {
      createMutation.mutate(value);
    },
  });

  const handleCopy = () => {
    if (createdKey) {
      navigator.clipboard.writeText(createdKey.key);
      setCopied(true);
      toast.success("Copied to clipboard");
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <>
      <Dialog open={!!createdKey} onOpenChange={(open) => !open && setCreatedKey(null)}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Key className="size-5" />
              API Key Created
            </DialogTitle>
            <DialogDescription>
              Copy this key now — it won't be shown again.
            </DialogDescription>
          </DialogHeader>
          {createdKey && (
            <div className="flex flex-col gap-4">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium">{createdKey.name}</span>
                <Badge variant={createdKey.permissions === "write" ? "default" : "secondary"}>
                  {createdKey.permissions === "write" ? "Read & Write" : "Read Only"}
                </Badge>
              </div>
              <div className="relative">
                <code className="block rounded-lg border bg-muted p-4 pr-12 font-mono text-sm break-all">
                  {createdKey.key}
                </code>
                <Button
                  variant="ghost"
                  size="icon"
                  className="absolute top-2 right-2 size-8"
                  onClick={handleCopy}
                >
                  {copied ? (
                    <Check className="size-4 text-green-600" />
                  ) : (
                    <Copy className="size-4" />
                  )}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Prefix: <span className="font-mono">{createdKey.key_prefix}</span>
              </p>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <Card>
        <CardHeader>
          <CardTitle>API Keys</CardTitle>
          <CardDescription>
            API keys allow external applications to access this site's content via
            the{" "}
            <a
              href="/api/v1/docs"
              target="_blank"
              rel="noopener noreferrer"
              className="underline"
            >
              REST API
            </a>{" "}
            and{" "}
            <a
              href="/api/graphql"
              target="_blank"
              rel="noopener noreferrer"
              className="underline"
            >
              GraphQL
            </a>
            .
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              apiKeyForm.handleSubmit();
            }}
            className="flex items-end gap-2"
          >
            <apiKeyForm.Field
              name="name"
              children={(field) => {
                const isInvalid =
                  field.state.meta.isTouched && !field.state.meta.isValid;
                return (
                  <Field data-invalid={isInvalid} className="flex-1">
                    <FieldLabel htmlFor={field.name}>Key Name</FieldLabel>
                    <Input
                      id={field.name}
                      name={field.name}
                      placeholder="e.g., Production Website"
                      value={field.state.value}
                      onBlur={field.handleBlur}
                      onChange={(e) => field.handleChange(e.target.value)}
                      aria-invalid={isInvalid}
                    />
                    {isInvalid && <FieldError errors={field.state.meta.errors} />}
                  </Field>
                );
              }}
            />
            <apiKeyForm.Field
              name="permissions"
              children={(field) => (
                <Field className="w-40">
                  <FieldLabel htmlFor="permissions">Permissions</FieldLabel>
                  <Select
                    value={field.state.value}
                    onValueChange={(v) => field.handleChange(v as "read" | "write")}
                  >
                    <SelectTrigger id="permissions">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="read">
                        <div className="flex items-center gap-2">
                          <Shield className="size-3.5" />
                          <span>Read Only</span>
                        </div>
                      </SelectItem>
                      <SelectItem value="write">
                        <div className="flex items-center gap-2">
                          <Shield className="size-3.5" />
                          <span>Read & Write</span>
                        </div>
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </Field>
              )}
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
                  <div className="flex items-center gap-3">
                    <Key className="size-4 text-muted-foreground" />
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-medium">{key.name}</span>
                        <Badge
                          variant={key.permissions === "write" ? "default" : "secondary"}
                          className="text-xs"
                        >
                          {key.permissions === "write" ? "R/W" : "R"}
                        </Badge>
                      </div>
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
                  </div>
                  <Button
                    variant="ghost"
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
    </>
  );
}
