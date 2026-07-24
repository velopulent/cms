import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Rocket } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useSiteRole } from "@/components/site-settings/use-site-role";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createDeployment,
  getDeploymentHistory,
  getDeployments,
  triggerDeployment,
} from "@/lib/api";

export const Route = createFileRoute("/_admin/sites/$siteId/deployments")({
  component: DeploymentsPage,
});

function DeploymentsPage() {
  const { siteId } = Route.useParams();
  const { canManage } = useSiteRole(siteId);
  const client = useQueryClient();
  const [label, setLabel] = useState("");
  const [provider, setProvider] = useState("custom");
  const [url, setUrl] = useState("");
  const { data = [] } = useQuery({
    queryKey: ["deployments", siteId],
    queryFn: () => getDeployments(siteId),
    refetchInterval: 5000,
  });
  const histories = useQueries({
    queries: data.map((trigger) => ({
      queryKey: ["deployment-history", siteId, trigger.id],
      queryFn: () => getDeploymentHistory(siteId, trigger.id),
      refetchInterval: 3000,
    })),
  });
  const create = useMutation({
    mutationFn: () =>
      createDeployment(siteId, {
        label,
        provider,
        url,
        headers: {},
        enabled: true,
        is_primary: data.length === 0,
        cooldown_seconds: 60,
        daily_quota: 20,
      }),
    onSuccess: () => {
      setLabel("");
      setUrl("");
      client.invalidateQueries({ queryKey: ["deployments", siteId] });
    },
    onError: (error: Error) => toast.error(error.message),
  });
  const trigger = useMutation({
    mutationFn: (id: string) => triggerDeployment(siteId, id),
    onSuccess: () => {
      toast.success("Deployment queued");
      client.invalidateQueries({ queryKey: ["deployment-history", siteId] });
    },
    onError: (error: Error) => toast.error(error.message),
  });

  return (
    <main className="mx-auto flex w-full max-w-4xl flex-col gap-6 p-4 sm:p-6">
      <div>
        <h1 className="text-2xl font-semibold">Deploy</h1>
        <p className="text-sm text-muted-foreground">
          Publish saved content through configured deployment triggers.
        </p>
      </div>
      <div className="grid gap-4">
        {data.map((item, index) => {
          const latest = histories[index]?.data?.[0];
          return (
            <Card key={item.id}>
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <CardTitle>{item.label}</CardTitle>
                    <CardDescription className="capitalize">
                      {item.provider} ·{" "}
                      {item.cooldown_seconds === 0
                        ? "No cooldown"
                        : `${item.cooldown_seconds}s cooldown`}{" "}
                      ·{" "}
                      {item.daily_quota === 0
                        ? "Unlimited"
                        : `${item.daily_quota}/day`}
                    </CardDescription>
                  </div>
                  <div className="flex gap-2">
                    {item.is_primary ? <Badge>Primary</Badge> : null}
                    <Badge variant={item.enabled ? "secondary" : "outline"}>
                      {item.enabled ? "Ready" : "Disabled"}
                    </Badge>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="flex flex-col gap-3">
                {latest ? (
                  <div className="text-sm">
                    <span className="text-muted-foreground">Latest: </span>
                    <Badge
                      variant={
                        latest.status === "succeeded"
                          ? "secondary"
                          : latest.status === "failed"
                            ? "destructive"
                            : "outline"
                      }
                    >
                      {latest.status}
                    </Badge>
                    {latest.error_category ? (
                      <span className="ml-2 text-muted-foreground">
                        {latest.error_category.replaceAll("_", " ")}
                      </span>
                    ) : null}
                  </div>
                ) : null}
                <Button
                  className="w-fit"
                  disabled={!item.enabled || trigger.isPending}
                  onClick={() => trigger.mutate(item.id)}
                >
                  <Rocket data-icon="inline-start" />
                  Deploy now
                </Button>
              </CardContent>
            </Card>
          );
        })}
        {data.length === 0 ? (
          <Card>
            <CardHeader>
              <CardTitle>No deployment trigger</CardTitle>
              <CardDescription>
                An operator must configure a deployment endpoint before editors
                can deploy.
              </CardDescription>
            </CardHeader>
          </Card>
        ) : null}
      </div>
      {canManage ? (
        <Card>
          <CardHeader>
            <CardTitle>Add deployment trigger</CardTitle>
            <CardDescription>
              Provider URL and headers are encrypted at rest. Defaults:
              60-second cooldown and 20 attempts per rolling day.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="deploy-label">Label</FieldLabel>
                <Input
                  id="deploy-label"
                  value={label}
                  onChange={(event) => setLabel(event.target.value)}
                  placeholder="Production"
                />
              </Field>
              <Field>
                <FieldLabel>Provider</FieldLabel>
                <Select
                  value={provider}
                  onValueChange={(value) => value && setProvider(value)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {[
                        "cloudflare",
                        "vercel",
                        "netlify",
                        "github",
                        "custom",
                      ].map((value) => (
                        <SelectItem
                          key={value}
                          value={value}
                          className="capitalize"
                        >
                          {value}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel htmlFor="deploy-url">Deployment URL</FieldLabel>
                <Input
                  id="deploy-url"
                  type="url"
                  value={url}
                  onChange={(event) => setUrl(event.target.value)}
                />
              </Field>
              <Button
                className="w-fit"
                disabled={!label.trim() || !url || create.isPending}
                onClick={() => create.mutate()}
              >
                Save trigger
              </Button>
            </FieldGroup>
          </CardContent>
        </Card>
      ) : null}
    </main>
  );
}
