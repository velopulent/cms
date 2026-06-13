import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Minus, Plus, Trash2, Webhook as WebhookIcon, Zap } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { z } from "zod";
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
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import {
  createWebhook,
  deleteWebhook,
  getWebhookDeliveries,
  getWebhooks,
  triggerWebhook,
  type Webhook,
  type WebhookDeliveryList,
} from "@/lib/api";

const webhookSchema = z.object({
  label: z.string().min(1, "Label is required"),
  url: z.string().url("Must be a valid URL"),
});

export function WebhooksSection({ siteId }: { siteId: string }) {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const [expandedWebhook, setExpandedWebhook] = useState<string | null>(null);
  const [triggeringId, setTriggeringId] = useState<string | null>(null);
  const [triggerResult, setTriggerResult] =
    useState<WebhookDeliveryList | null>(null);
  const [headerEntries, setHeaderEntries] = useState<
    { id: string; key: string; value: string }[]
  >([{ id: crypto.randomUUID(), key: "", value: "" }]);

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
      setHeaderEntries([{ id: crypto.randomUUID(), key: "", value: "" }]);
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
    setHeaderEntries([
      ...headerEntries,
      { id: crypto.randomUUID(), key: "", value: "" },
    ]);
  };

  const removeHeaderEntry = (id: string) => {
    setHeaderEntries(headerEntries.filter((entry) => entry.id !== id));
  };

  const updateHeaderEntry = (
    id: string,
    field: "key" | "value",
    val: string,
  ) => {
    setHeaderEntries(
      headerEntries.map((entry) =>
        entry.id === id ? { ...entry, [field]: val } : entry,
      ),
    );
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
              {headerEntries.map((entry) => (
                <div key={entry.id} className="flex items-center gap-2">
                  <Input
                    placeholder="Header name"
                    value={entry.key}
                    onChange={(e) =>
                      updateHeaderEntry(entry.id, "key", e.target.value)
                    }
                    className="flex-1"
                  />
                  <Input
                    placeholder="Value"
                    value={entry.value}
                    onChange={(e) =>
                      updateHeaderEntry(entry.id, "value", e.target.value)
                    }
                    className="flex-1"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-8 shrink-0"
                    onClick={() => removeHeaderEntry(entry.id)}
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
                  setHeaderEntries([
                    { id: crypto.randomUUID(), key: "", value: "" },
                  ]);
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
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
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
            <Button
              size="sm"
              className="w-fit"
              onClick={() => setShowCreate(true)}
            >
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
                  <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                    <div className="flex min-w-0 flex-col gap-0.5">
                      <span className="font-medium">{wh.label}</span>
                      <span className="text-sm text-muted-foreground font-mono truncate">
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
