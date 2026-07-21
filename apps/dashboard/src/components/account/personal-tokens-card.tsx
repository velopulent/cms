import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, KeyRound, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  createPersonalToken,
  getPersonalTokens,
  revokePersonalToken,
} from "@/lib/api";

const SCOPES = [
  "site.read",
  "site.settings.read",
  "content.read",
  "content.write",
  "files.read",
  "files.write",
  "schema.read",
  "schema.write",
  "webhooks.read",
  "webhooks.write",
  "webhooks.trigger",
  "deployments.read",
  "deployments.write",
  "deployments.trigger",
  "mcp.use",
];

export function PersonalTokensCard() {
  const client = useQueryClient();
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<string[]>([
    "site.read",
    "content.read",
    "content.write",
    "files.read",
    "files.write",
    "schema.read",
    "deployments.read",
    "deployments.trigger",
    "mcp.use",
  ]);
  const [secret, setSecret] = useState<string | null>(null);
  const { data = [] } = useQuery({
    queryKey: ["personal-tokens"],
    queryFn: getPersonalTokens,
  });
  const create = useMutation({
    mutationFn: () => createPersonalToken({ name, scopes, expires_at: null }),
    onSuccess: (value) => {
      setSecret(value.token);
      setName("");
      client.invalidateQueries({ queryKey: ["personal-tokens"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const revoke = useMutation({
    mutationFn: revokePersonalToken,
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ["personal-tokens"] }),
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Personal access tokens</CardTitle>
          <CardDescription>
            User-owned credentials for MCP, CLI, and API access. Effective
            access always remains limited by your current role.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-6">
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="pat-name">Token name</FieldLabel>
              <Input
                id="pat-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Claude Desktop"
              />
            </Field>
            <FieldSet>
              <FieldLegend>Capabilities</FieldLegend>
              <div className="grid gap-3 sm:grid-cols-2">
                {SCOPES.map((scope) => (
                  <Field key={scope} orientation="horizontal">
                    <Checkbox
                      checked={scopes.includes(scope)}
                      onCheckedChange={(checked) =>
                        setScopes((current) =>
                          checked
                            ? [...current, scope]
                            : current.filter((item) => item !== scope),
                        )
                      }
                    />
                    <FieldLabel>{scope}</FieldLabel>
                  </Field>
                ))}
              </div>
            </FieldSet>
          </FieldGroup>
          <Button
            className="w-fit"
            disabled={!name.trim() || scopes.length === 0 || create.isPending}
            onClick={() => create.mutate()}
          >
            <KeyRound data-icon="inline-start" />
            Create token
          </Button>
          <div className="flex flex-col gap-3">
            {data.map((token) => (
              <div
                key={token.id}
                className="flex items-start justify-between gap-3 rounded-lg border p-3"
              >
                <div className="flex flex-col gap-1">
                  <p className="font-medium">{token.name}</p>
                  <p className="font-mono text-xs text-muted-foreground">
                    {token.token_prefix}…
                  </p>
                  <div className="flex flex-wrap gap-1">
                    {token.scopes.map((scope) => (
                      <Badge key={scope} variant="secondary">
                        {scope}
                      </Badge>
                    ))}
                  </div>
                </div>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`Revoke ${token.name}`}
                  onClick={() => revoke.mutate(token.id)}
                >
                  <Trash2 />
                </Button>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
      <Dialog
        open={secret !== null}
        onOpenChange={(open) => {
          if (!open) setSecret(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Copy personal access token</DialogTitle>
            <DialogDescription>
              This secret is shown once. Store it securely.
            </DialogDescription>
          </DialogHeader>
          <div className="flex gap-2">
            <Input readOnly value={secret ?? ""} className="font-mono" />
            <Button
              size="icon"
              variant="outline"
              onClick={() => {
                navigator.clipboard.writeText(secret ?? "");
                toast.success("Token copied");
              }}
            >
              <Copy />
            </Button>
          </div>
          <DialogFooter>
            <Button onClick={() => setSecret(null)}>Done</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
