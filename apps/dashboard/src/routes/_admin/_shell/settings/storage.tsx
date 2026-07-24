import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { Database, Trash2 } from "lucide-react";
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
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  createStorageProfile,
  deleteStorageProfile,
  getMe,
  getStorageProfiles,
  isOperator,
} from "@/lib/api";
export const Route = createFileRoute("/_admin/_shell/settings/storage")({
  beforeLoad: async ({ context }) => {
    const me = await context.queryClient.ensureQueryData({
      queryKey: ["me"],
      queryFn: getMe,
    });
    if (!isOperator(me.instance_role)) throw redirect({ to: "/" });
  },
  component: StorageProfiles,
});
function StorageProfiles() {
  const client = useQueryClient();
  const [name, setName] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [region, setRegion] = useState("");
  const [bucket, setBucket] = useState("");
  const [key, setKey] = useState("");
  const [secret, setSecret] = useState("");
  const { data = [] } = useQuery({
    queryKey: ["storage-profiles"],
    queryFn: getStorageProfiles,
  });
  const create = useMutation({
    mutationFn: () =>
      createStorageProfile({
        name,
        endpoint,
        region: region || null,
        bucket,
        public_url: null,
        access_key_id: key,
        secret_access_key: secret,
      }),
    onSuccess: () => {
      setName("");
      setEndpoint("");
      setRegion("");
      setBucket("");
      setKey("");
      setSecret("");
      client.invalidateQueries({ queryKey: ["storage-profiles"] });
    },
    onError: (e: Error) => toast.error(e.message),
  });
  const remove = useMutation({
    mutationFn: deleteStorageProfile,
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ["storage-profiles"] }),
    onError: (e: Error) => toast.error(e.message),
  });
  return (
    <div className="flex flex-col gap-6">
      <div className="grid gap-4 sm:grid-cols-2">
        {data.map((profile) => (
          <Card key={profile.id}>
            <CardHeader>
              <div className="flex justify-between gap-3">
                <div>
                  <CardTitle>{profile.name}</CardTitle>
                  <CardDescription>
                    {profile.kind === "filesystem"
                      ? "Instance data directory"
                      : profile.bucket}
                  </CardDescription>
                </div>
                <Badge variant="secondary">{profile.kind.toUpperCase()}</Badge>
              </div>
            </CardHeader>
            <CardContent className="flex justify-between gap-3">
              <p className="text-sm text-muted-foreground">
                {profile.endpoint ?? "Built-in provider"}
              </p>
              {!profile.immutable && (
                <Button
                  size="icon"
                  variant="ghost"
                  onClick={() => remove.mutate(profile.id)}
                >
                  <Trash2 />
                </Button>
              )}
            </CardContent>
          </Card>
        ))}
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Add S3-compatible profile</CardTitle>
          <CardDescription>
            Save reusable bucket credentials for sites and backups. Credentials
            are encrypted and never returned.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="storage-name">Short name</FieldLabel>
              <Input
                id="storage-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Production media"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="storage-endpoint">Endpoint</FieldLabel>
              <Input
                id="storage-endpoint"
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
                placeholder="https://..."
              />
            </Field>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel htmlFor="storage-region">Region</FieldLabel>
                <Input
                  id="storage-region"
                  value={region}
                  onChange={(e) => setRegion(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="storage-bucket">Bucket</FieldLabel>
                <Input
                  id="storage-bucket"
                  value={bucket}
                  onChange={(e) => setBucket(e.target.value)}
                />
              </Field>
            </div>
            <Field>
              <FieldLabel htmlFor="storage-key">Access key ID</FieldLabel>
              <Input
                id="storage-key"
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="storage-secret">
                Secret access key
              </FieldLabel>
              <Input
                id="storage-secret"
                type="password"
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
              />
            </Field>
            <Button
              className="w-fit"
              disabled={
                !name ||
                !endpoint ||
                !bucket ||
                !key ||
                !secret ||
                create.isPending
              }
              onClick={() => create.mutate()}
            >
              <Database data-icon="inline-start" />
              Save profile
            </Button>
          </FieldGroup>
        </CardContent>
      </Card>
    </div>
  );
}
