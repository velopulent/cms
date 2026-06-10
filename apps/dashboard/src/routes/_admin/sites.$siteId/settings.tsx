import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  Check,
  Copy,
  Key,
  Minus,
  Plus,
  Shield,
  Trash2,
  Webhook as WebhookIcon,
  Zap,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  createWebhook,
  deleteApiKey,
  deleteWebhook,
  getApiKeys,
  getSite,
  getSiteMembers,
  getSites,
  getWebhookDeliveries,
  getWebhooks,
  inviteMember,
  removeMember,
  transferOwnership,
  triggerWebhook,
  updateMemberRole,
  updateSite,
  type Webhook,
  type WebhookDeliveryList,
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
  const { data: sites } = useQuery({
    queryKey: ["sites"],
    queryFn: getSites,
  });
  const siteRole = sites?.find((item) => item.id === siteId)?.role ?? "viewer";
  const canManage = siteRole === "owner" || siteRole === "admin";

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
    mutationFn: ({ name }: { name: string }) => updateSite(siteId, { name }),
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
                        disabled={!canManage}
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
          disabled={!canManage || updateMutation.isPending}
        >
          {!canManage
            ? "Admin access required"
            : updateMutation.isPending
              ? "Saving..."
              : "Save Changes"}
        </Button>
      </form>

      <MembersSection siteId={siteId} currentRole={siteRole} />
      {canManage && <ApiKeysSection siteId={siteId} />}
      {canManage && <WebhooksSection siteId={siteId} />}
    </div>
  );
}

function MembersSection({
  siteId,
  currentRole,
}: {
  siteId: string;
  currentRole: "owner" | "admin" | "editor" | "viewer";
}) {
  const queryClient = useQueryClient();
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<"admin" | "editor" | "viewer">("viewer");
  const { data: members, isLoading } = useQuery({
    queryKey: ["site-members", siteId],
    queryFn: () => getSiteMembers(siteId),
  });
  const canManage = currentRole === "owner" || currentRole === "admin";

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["site-members", siteId] });
  const inviteMutation = useMutation({
    mutationFn: () => inviteMember(siteId, { username, role }),
    onSuccess: () => {
      invalidate();
      setUsername("");
      toast.success("Member added");
    },
    onError: (error: Error) => toast.error(error.message),
  });
  const roleMutation = useMutation({
    mutationFn: ({ userId, nextRole }: { userId: string; nextRole: string }) =>
      updateMemberRole(siteId, userId, nextRole),
    onSuccess: () => {
      invalidate();
      toast.success("Role updated");
    },
    onError: (error: Error) => toast.error(error.message),
  });
  const removeMutation = useMutation({
    mutationFn: (userId: string) => removeMember(siteId, userId),
    onSuccess: () => {
      invalidate();
      toast.success("Member removed");
    },
    onError: (error: Error) => toast.error(error.message),
  });
  const transferMutation = useMutation({
    mutationFn: (userId: string) => transferOwnership(siteId, userId),
    onSuccess: () => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["sites"] });
      toast.success("Ownership transferred");
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Members and ownership</CardTitle>
        <CardDescription>
          Editors manage content and files. Admins also manage schemas,
          webhooks, keys, and site settings.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {canManage && (
          <form
            className="flex flex-col gap-3 sm:flex-row sm:items-end"
            onSubmit={(event) => {
              event.preventDefault();
              inviteMutation.mutate();
            }}
          >
            <Field className="flex-1">
              <FieldLabel htmlFor="member-username">Username</FieldLabel>
              <Input
                id="member-username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                required
              />
            </Field>
            <Field className="sm:w-44">
              <FieldLabel htmlFor="member-role">Role</FieldLabel>
              <Select
                value={role}
                onValueChange={(value) =>
                  setRole(value as "admin" | "editor" | "viewer")
                }
              >
                <SelectTrigger id="member-role">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {currentRole === "owner" && (
                    <SelectItem value="admin">Admin</SelectItem>
                  )}
                  <SelectItem value="editor">Editor</SelectItem>
                  <SelectItem value="viewer">Viewer</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Button type="submit" disabled={inviteMutation.isPending}>
              Add member
            </Button>
          </form>
        )}

        {isLoading ? (
          <Skeleton className="h-24 w-full" />
        ) : (
          <div className="overflow-x-auto rounded-md border">
            <table className="w-full min-w-2xl text-sm">
              <thead className="border-b bg-muted/50 text-left">
                <tr>
                  <th className="p-3 font-medium">Member</th>
                  <th className="p-3 font-medium">Role</th>
                  <th className="p-3 text-right font-medium">Actions</th>
                </tr>
              </thead>
              <tbody>
                {members?.map((member) => {
                  const targetIsAdmin = member.role === "admin";
                  const editable =
                    canManage &&
                    member.role !== "owner" &&
                    (!targetIsAdmin || currentRole === "owner");
                  return (
                    <tr className="border-b last:border-0" key={member.id}>
                      <td className="p-3">
                        <div className="font-medium">{member.username}</div>
                        <div className="text-muted-foreground">
                          {member.email}
                        </div>
                      </td>
                      <td className="p-3">
                        {editable ? (
                          <Select
                            value={member.role}
                            onValueChange={(nextRole) => {
                              if (nextRole) {
                                roleMutation.mutate({
                                  userId: member.user_id,
                                  nextRole,
                                });
                              }
                            }}
                          >
                            <SelectTrigger className="w-36">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              {currentRole === "owner" && (
                                <SelectItem value="admin">Admin</SelectItem>
                              )}
                              <SelectItem value="editor">Editor</SelectItem>
                              <SelectItem value="viewer">Viewer</SelectItem>
                            </SelectContent>
                          </Select>
                        ) : (
                          <Badge variant="secondary">{member.role}</Badge>
                        )}
                      </td>
                      <td className="p-3 text-right">
                        <div className="flex justify-end gap-2">
                          {currentRole === "owner" &&
                            member.role !== "owner" && (
                              <AlertDialog>
                                <AlertDialogTrigger
                                  render={
                                    <Button variant="outline" size="sm" />
                                  }
                                >
                                  Transfer ownership
                                </AlertDialogTrigger>
                                <AlertDialogContent>
                                  <AlertDialogHeader>
                                    <AlertDialogTitle>
                                      Transfer site ownership?
                                    </AlertDialogTitle>
                                    <AlertDialogDescription>
                                      {member.username} becomes owner. Your role
                                      becomes admin.
                                    </AlertDialogDescription>
                                  </AlertDialogHeader>
                                  <AlertDialogFooter>
                                    <AlertDialogCancel>
                                      Cancel
                                    </AlertDialogCancel>
                                    <AlertDialogAction
                                      onClick={() =>
                                        transferMutation.mutate(member.user_id)
                                      }
                                    >
                                      Transfer ownership
                                    </AlertDialogAction>
                                  </AlertDialogFooter>
                                </AlertDialogContent>
                              </AlertDialog>
                            )}
                          {editable && (
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={removeMutation.isPending}
                              onClick={() =>
                                removeMutation.mutate(member.user_id)
                              }
                            >
                              Remove
                            </Button>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
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
    mutationFn: ({
      name,
      permissions,
    }: {
      name: string;
      permissions: string;
    }) => createApiKey(siteId, name, permissions),
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
      <Dialog
        open={!!createdKey}
        onOpenChange={(open) => !open && setCreatedKey(null)}
      >
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
                <Badge
                  variant={
                    createdKey.permissions === "write" ? "default" : "secondary"
                  }
                >
                  {createdKey.permissions === "write"
                    ? "Read & Write"
                    : "Read Only"}
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
                Prefix:{" "}
                <span className="font-mono">{createdKey.key_prefix}</span>
              </p>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <Card>
        <CardHeader>
          <CardTitle>API Keys</CardTitle>
          <CardDescription>
            API keys allow external applications to access this site's content
            via the{" "}
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
                    {isInvalid && (
                      <FieldError errors={field.state.meta.errors} />
                    )}
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
                    onValueChange={(v) =>
                      field.handleChange(v as "read" | "write")
                    }
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
                          variant={
                            key.permissions === "write"
                              ? "default"
                              : "secondary"
                          }
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

const webhookSchema = z.object({
  label: z.string().min(1, "Label is required"),
  url: z.string().url("Must be a valid URL"),
});

function WebhooksSection({ siteId }: { siteId: string }) {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [expandedWebhook, setExpandedWebhook] = useState<string | null>(null);
  const [triggeringId, setTriggeringId] = useState<string | null>(null);
  const [triggerResult, setTriggerResult] =
    useState<WebhookDeliveryList | null>(null);
  const [headerEntries, setHeaderEntries] = useState<
    { key: string; value: string }[]
  >([{ key: "", value: "" }]);

  const { data: webhooks, isLoading } = useQuery({
    queryKey: ["webhooks", siteId],
    queryFn: () => getWebhooks(siteId),
  });

  const { data: deliveries } = useQuery({
    queryKey: ["webhook-deliveries", siteId, expandedWebhook],
    queryFn: () =>
      expandedWebhook
        ? getWebhookDeliveries(siteId, expandedWebhook)
        : Promise.reject(new Error("No webhook selected")),
    enabled: !!expandedWebhook,
  });

  const createMutation = useMutation({
    mutationFn: (data: {
      label: string;
      url: string;
      headers?: Record<string, string>;
    }) => createWebhook(siteId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["webhooks", siteId] });
      setShowCreate(false);
      webhookForm.reset();
      setHeaderEntries([{ key: "", value: "" }]);
      toast.success("Webhook created");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteWebhook(siteId, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["webhooks", siteId] });
      toast.success("Webhook deleted");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const triggerMutation = useMutation({
    mutationFn: (id: string) => triggerWebhook(siteId, id),
    onSuccess: (delivery) => {
      setTriggerResult(delivery);
      queryClient.invalidateQueries({
        queryKey: ["webhook-deliveries", siteId, expandedWebhook],
      });
      if (delivery.status === "success") {
        toast.success("Webhook triggered successfully");
      } else {
        toast.error(
          `Webhook delivery failed with status ${delivery.status_code}`,
        );
      }
      setTriggeringId(null);
    },
    onError: (err: Error) => {
      toast.error(err.message);
      setTriggeringId(null);
    },
  });

  const webhookForm = useForm({
    defaultValues: {
      label: "",
      url: "",
    },
    validators: {
      onSubmit: webhookSchema,
    },
    onSubmit: async ({ value }) => {
      const headers: Record<string, string> = {};
      for (const entry of headerEntries) {
        if (entry.key.trim() && entry.value.trim()) {
          headers[entry.key.trim()] = entry.value.trim();
        }
      }
      createMutation.mutate({
        ...value,
        headers: Object.keys(headers).length > 0 ? headers : undefined,
      });
    },
  });

  const handleTrigger = (id: string) => {
    setTriggeringId(id);
    setTriggerResult(null);
    triggerMutation.mutate(id);
  };

  const addHeaderEntry = () => {
    setHeaderEntries([...headerEntries, { key: "", value: "" }]);
  };

  const removeHeaderEntry = (index: number) => {
    setHeaderEntries(headerEntries.filter((_, i) => i !== index));
  };

  const updateHeaderEntry = (
    index: number,
    field: "key" | "value",
    val: string,
  ) => {
    const updated = [...headerEntries];
    updated[index] = { ...updated[index], [field]: val };
    setHeaderEntries(updated);
  };

  return (
    <>
      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <WebhookIcon className="size-5" />
              Create Webhook
            </DialogTitle>
            <DialogDescription>
              Add a webhook to trigger deployments on Cloudflare Pages, Vercel,
              Netlify, or any service that supports deploy hooks.
            </DialogDescription>
          </DialogHeader>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              webhookForm.handleSubmit();
            }}
            className="flex flex-col gap-4"
          >
            <webhookForm.Field
              name="label"
              children={(field) => {
                const isInvalid =
                  field.state.meta.isTouched && !field.state.meta.isValid;
                return (
                  <Field data-invalid={isInvalid}>
                    <FieldLabel htmlFor={field.name}>Label</FieldLabel>
                    <Input
                      id={field.name}
                      name={field.name}
                      placeholder="e.g., Production Deploy"
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
            <webhookForm.Field
              name="url"
              children={(field) => {
                const isInvalid =
                  field.state.meta.isTouched && !field.state.meta.isValid;
                return (
                  <Field data-invalid={isInvalid}>
                    <FieldLabel htmlFor={field.name}>URL</FieldLabel>
                    <Input
                      id={field.name}
                      name={field.name}
                      placeholder="https://api.example.com/deploy"
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
            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between">
                <FieldLabel>Custom Headers</FieldLabel>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-6 px-2 text-xs"
                  onClick={addHeaderEntry}
                >
                  <Plus className="size-3 mr-1" />
                  Add Header
                </Button>
              </div>
              {headerEntries.map((entry, index) => (
                <div key={index} className="flex items-center gap-2">
                  <Input
                    placeholder="Header name"
                    value={entry.key}
                    onChange={(e) =>
                      updateHeaderEntry(index, "key", e.target.value)
                    }
                    className="flex-1"
                  />
                  <Input
                    placeholder="Value"
                    value={entry.value}
                    onChange={(e) =>
                      updateHeaderEntry(index, "value", e.target.value)
                    }
                    className="flex-1"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-8 shrink-0"
                    onClick={() => removeHeaderEntry(index)}
                    disabled={headerEntries.length <= 1}
                  >
                    <Minus className="size-3.5" />
                  </Button>
                </div>
              ))}
              <p className="text-xs text-muted-foreground">
                Headers like Authorization are sent with every webhook request.
                Leave empty to skip.
              </p>
            </div>
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => {
                  setShowCreate(false);
                  setHeaderEntries([{ key: "", value: "" }]);
                }}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={createMutation.isPending}>
                {createMutation.isPending ? "Creating..." : "Create Webhook"}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                <WebhookIcon className="size-5" />
                Webhooks
              </CardTitle>
              <CardDescription>
                Trigger deployments to Cloudflare Pages, Vercel, Netlify, and
                other services.
              </CardDescription>
            </div>
            <Button size="sm" onClick={() => setShowCreate(true)}>
              <Plus className="size-4 mr-1" />
              Add Webhook
            </Button>
          </div>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {isLoading ? (
            <Skeleton className="h-20 w-full" />
          ) : webhooks && webhooks.length > 0 ? (
            <div className="flex flex-col gap-3">
              {webhooks.map((wh: Webhook) => (
                <div
                  key={wh.id}
                  className="flex flex-col gap-2 rounded-md border p-3"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex flex-col gap-0.5">
                      <span className="font-medium">{wh.label}</span>
                      <span className="text-sm text-muted-foreground font-mono truncate max-w-md">
                        {wh.url}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => handleTrigger(wh.id)}
                        disabled={triggeringId === wh.id}
                      >
                        <Zap className="size-3.5 mr-1" />
                        {triggeringId === wh.id ? "Triggering..." : "Deploy"}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() =>
                          setExpandedWebhook(
                            expandedWebhook === wh.id ? null : wh.id,
                          )
                        }
                      >
                        {expandedWebhook === wh.id ? "Hide" : "History"}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="text-destructive hover:text-destructive"
                        onClick={() => deleteMutation.mutate(wh.id)}
                        disabled={deleteMutation.isPending}
                      >
                        <Trash2 className="size-3.5" />
                      </Button>
                    </div>
                  </div>

                  {triggerResult && triggerResult.webhook_id === wh.id && (
                    <div
                      className={`rounded-md border p-2 text-sm ${
                        triggerResult.status === "success"
                          ? "border-green-200 bg-green-50 text-green-800 dark:border-green-800 dark:bg-green-950 dark:text-green-300"
                          : "border-red-200 bg-red-50 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-300"
                      }`}
                    >
                      <div className="flex items-center gap-2">
                        <span className="font-medium">
                          {triggerResult.status === "success"
                            ? "Success"
                            : "Failed"}
                        </span>
                        {triggerResult.status_code && (
                          <Badge variant="outline" className="text-xs">
                            HTTP {triggerResult.status_code}
                          </Badge>
                        )}
                        {triggerResult.duration_ms != null && (
                          <span className="text-xs text-muted-foreground">
                            {triggerResult.duration_ms}ms
                          </span>
                        )}
                      </div>
                    </div>
                  )}

                  {expandedWebhook === wh.id && deliveries && (
                    <div className="flex flex-col gap-1 mt-1">
                      <div className="text-xs font-medium text-muted-foreground">
                        Recent Deliveries
                      </div>
                      {deliveries.items.length === 0 ? (
                        <p className="text-xs text-muted-foreground">
                          No deliveries yet.
                        </p>
                      ) : (
                        deliveries.items.map((d: WebhookDeliveryList) => (
                          <div
                            key={d.id}
                            className="flex items-center justify-between text-xs py-1 border-b last:border-0"
                          >
                            <div className="flex items-center gap-2">
                              <Badge
                                variant={
                                  d.status === "success"
                                    ? "default"
                                    : "destructive"
                                }
                                className="text-xs px-1.5 py-0"
                              >
                                {d.status}
                              </Badge>
                              {d.status_code && (
                                <span className="text-muted-foreground">
                                  HTTP {d.status_code}
                                </span>
                              )}
                              {d.duration_ms != null && (
                                <span className="text-muted-foreground">
                                  {d.duration_ms}ms
                                </span>
                              )}
                            </div>
                            <span className="text-muted-foreground">
                              {new Date(d.triggered_at).toLocaleString()}
                            </span>
                          </div>
                        ))
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              No webhooks yet. Add one to trigger deployments on content
              changes.
            </p>
          )}
        </CardContent>
      </Card>
    </>
  );
}
