import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { toast } from "sonner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupInput } from "@/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  getInstanceSettings,
  type InstanceSettings,
  updateInstanceSettingsSection,
} from "@/lib/api";

export function SettingsLoading() {
  return (
    <Card>
      <CardHeader>
        <Skeleton className="h-6 w-40" />
        <Skeleton className="h-4 w-72" />
      </CardHeader>
      <CardContent className="space-y-5">
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-2/3" />
      </CardContent>
    </Card>
  );
}

function useSettings() {
  return useQuery({
    queryKey: ["instance-settings"],
    queryFn: getInstanceSettings,
  });
}

function useSave(section: "general" | "security" | "storage" | "backups") {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: unknown) => updateInstanceSettingsSection(section, data),
    onSuccess: (data) => {
      queryClient.setQueryData(["instance-settings"], data);
      toast.success("Settings saved");
    },
    onError: (error: Error) => toast.error(error.message),
  });
}

function ToggleField({
  label,
  description,
  checked,
  onCheckedChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <Field orientation="horizontal">
      <div className="flex-1">
        <FieldLabel>{label}</FieldLabel>
        <FieldDescription>{description}</FieldDescription>
      </div>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </Field>
  );
}

function TextField({
  label,
  value,
  onChange,
  placeholder,
  type = "text",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: string;
}) {
  return (
    <Field>
      <FieldLabel>{label}</FieldLabel>
      <Input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
    </Field>
  );
}

export function GeneralSettingsPanel() {
  const query = useSettings();
  if (query.isLoading) return <SettingsLoading />;
  if (!query.data)
    return <p className="text-sm text-destructive">Unable to load settings.</p>;
  return (
    <GeneralForm
      key={JSON.stringify(query.data.general)}
      settings={query.data}
    />
  );
}

function GeneralForm({ settings }: { settings: InstanceSettings }) {
  const [form, setForm] = useState(settings.general);
  const save = useSave("general");
  return (
    <Card>
      <CardHeader>
        <CardTitle>General</CardTitle>
        <CardDescription>
          Public behavior and user-facing runtime limits.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <TextField
            label="Public URL"
            value={form.public_url ?? ""}
            placeholder="https://cms.example.com"
            onChange={(value) =>
              setForm({ ...form, public_url: value || null })
            }
          />
          <div className="grid gap-6 sm:grid-cols-2">
            <TextField
              label="Session lifetime (hours)"
              type="number"
              value={String(form.session_lifetime_hours)}
              onChange={(value) =>
                setForm({ ...form, session_lifetime_hours: Number(value) })
              }
            />
            <TextField
              label="Upload limit (MB)"
              type="number"
              value={String(form.upload_limit_mb)}
              onChange={(value) =>
                setForm({ ...form, upload_limit_mb: Number(value) })
              }
            />
          </div>
          <ToggleField
            label="Public registration"
            description="Allow visitors to create accounts."
            checked={form.public_registration}
            onCheckedChange={(value) =>
              setForm({ ...form, public_registration: value })
            }
          />
          <ToggleField
            label="MCP endpoint"
            description="Expose the authenticated MCP HTTP endpoint."
            checked={form.mcp_enabled}
            onCheckedChange={(value) =>
              setForm({ ...form, mcp_enabled: value })
            }
          />
        </FieldGroup>
      </CardContent>
      <CardFooter className="justify-end border-t">
        <Button disabled={save.isPending} onClick={() => save.mutate(form)}>
          {save.isPending ? "Saving…" : "Save general"}
        </Button>
      </CardFooter>
    </Card>
  );
}

export function SecuritySettingsPanel() {
  const query = useSettings();
  if (query.isLoading) return <SettingsLoading />;
  if (!query.data)
    return <p className="text-sm text-destructive">Unable to load settings.</p>;
  return (
    <SecurityForm
      key={JSON.stringify(query.data.security)}
      settings={query.data}
    />
  );
}

function lines(value: string) {
  return value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function SecurityForm({ settings }: { settings: InstanceSettings }) {
  const initial = settings.security;
  const [form, setForm] = useState(initial);
  const save = useSave("security");
  return (
    <Card>
      <CardHeader>
        <CardTitle>Security</CardTitle>
        <CardDescription>
          Browser, proxy, webhook, and MCP trust boundaries.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <TextField
            label="Allowed origins"
            value={form.allowed_origins.join("\n")}
            placeholder="One origin per line"
            onChange={(value) =>
              setForm({ ...form, allowed_origins: lines(value) })
            }
          />
          <TextField
            label="MCP allowed hosts"
            value={form.mcp_allowed_hosts.join("\n")}
            onChange={(value) =>
              setForm({ ...form, mcp_allowed_hosts: lines(value) })
            }
          />
          <TextField
            label="MCP allowed origins"
            value={form.mcp_allowed_origins.join("\n")}
            onChange={(value) =>
              setForm({ ...form, mcp_allowed_origins: lines(value) })
            }
          />
          <ToggleField
            label="Secure cookies"
            description="Only send authentication cookies over HTTPS."
            checked={form.secure_cookies}
            onCheckedChange={(value) =>
              setForm({ ...form, secure_cookies: value })
            }
          />
          <ToggleField
            label="Trust proxy headers"
            description="Honor forwarded client addresses from your reverse proxy."
            checked={form.trusted_proxy_headers}
            onCheckedChange={(value) =>
              setForm({ ...form, trusted_proxy_headers: value })
            }
          />
          <ToggleField
            label="Private webhook targets"
            description="Permit delivery to private and loopback networks."
            checked={form.private_webhook_targets}
            onCheckedChange={(value) =>
              setForm({ ...form, private_webhook_targets: value })
            }
          />
        </FieldGroup>
      </CardContent>
      <CardFooter className="justify-end border-t">
        <Button disabled={save.isPending} onClick={() => save.mutate(form)}>
          {save.isPending ? "Saving…" : "Save security"}
        </Button>
      </CardFooter>
    </Card>
  );
}

type ProviderSection =
  | InstanceSettings["storage"]
  | InstanceSettings["backups"];

function Credentials({
  configured,
  masked,
  accessKey,
  secretKey,
  setAccessKey,
  setSecretKey,
}: {
  configured: boolean;
  masked: string | null;
  accessKey: string;
  secretKey: string;
  setAccessKey: (value: string) => void;
  setSecretKey: (value: string) => void;
}) {
  return (
    <div className="grid gap-6 sm:grid-cols-2">
      <Field>
        <div className="flex items-center gap-2">
          <FieldLabel>Access key ID</FieldLabel>
          {configured && <Badge variant="secondary">Configured {masked}</Badge>}
        </div>
        <InputGroup>
          <InputGroupInput
            value={accessKey}
            autoComplete="off"
            placeholder={configured ? "Leave empty to retain" : "Required"}
            onChange={(event) => setAccessKey(event.target.value)}
          />
        </InputGroup>
      </Field>
      <Field>
        <FieldLabel>Secret access key</FieldLabel>
        <InputGroup>
          <InputGroupInput
            type="password"
            value={secretKey}
            autoComplete="new-password"
            placeholder={configured ? "Leave empty to retain" : "Required"}
            onChange={(event) => setSecretKey(event.target.value)}
          />
        </InputGroup>
      </Field>
    </div>
  );
}

function ProviderFields({
  section,
  onChange,
}: {
  section: ProviderSection;
  onChange: (next: ProviderSection) => void;
}) {
  const provider =
    "provider" in section ? section.provider : section.destination;
  const setProvider = (value: "filesystem" | "s3") =>
    onChange(
      "provider" in section
        ? { ...section, provider: value }
        : { ...section, destination: value },
    );
  return (
    <>
      <Field>
        <FieldLabel>Provider</FieldLabel>
        <Select
          value={provider}
          onValueChange={(value) =>
            value && setProvider(value as "filesystem" | "s3")
          }
        >
          <SelectTrigger className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="filesystem">Filesystem</SelectItem>
            <SelectItem value="s3">Amazon S3 compatible</SelectItem>
          </SelectContent>
        </Select>
      </Field>
      {provider === "s3" && (
        <div className="grid gap-6 sm:grid-cols-2">
          <TextField
            label="Bucket"
            value={section.bucket ?? ""}
            onChange={(value) =>
              onChange({ ...section, bucket: value || null })
            }
          />
          <TextField
            label="Region"
            value={section.region ?? ""}
            onChange={(value) =>
              onChange({ ...section, region: value || null })
            }
          />
          <TextField
            label="Endpoint"
            value={section.endpoint ?? ""}
            placeholder="Optional for AWS"
            onChange={(value) =>
              onChange({ ...section, endpoint: value || null })
            }
          />
          {"public_url" in section && (
            <TextField
              label="Public URL"
              value={section.public_url ?? ""}
              onChange={(value) =>
                onChange({ ...section, public_url: value || null })
              }
            />
          )}
        </div>
      )}
    </>
  );
}

export function StorageSettingsPanel() {
  const query = useSettings();
  if (query.isLoading) return <SettingsLoading />;
  if (!query.data)
    return <p className="text-sm text-destructive">Unable to load settings.</p>;
  return (
    <StorageForm
      key={JSON.stringify(query.data.storage)}
      settings={query.data}
    />
  );
}

function StorageForm({ settings }: { settings: InstanceSettings }) {
  const [form, setForm] = useState(settings.storage);
  const [accessKey, setAccessKey] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [confirming, setConfirming] = useState(false);
  const save = useSave("storage");
  const targetChanged =
    (settings.storage.provider === "s3" || form.provider === "s3") &&
    (settings.storage.provider !== form.provider ||
      settings.storage.bucket !== form.bucket ||
      settings.storage.region !== form.region ||
      settings.storage.endpoint !== form.endpoint);
  const submit = (confirmed = false) =>
    save.mutate(
      {
        ...form,
        access_key_id: accessKey || null,
        secret_access_key: secretKey || null,
        confirm_target_change: confirmed,
      },
      {
        onSuccess: () => {
          setAccessKey("");
          setSecretKey("");
        },
      },
    );
  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Storage</CardTitle>
          <CardDescription>
            One global provider. Filesystem paths stay fixed under the data
            root.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <ProviderFields
              section={form}
              onChange={(next) => setForm(next as InstanceSettings["storage"])}
            />
            {form.provider === "s3" && (
              <Credentials
                configured={settings.storage_credentials.configured}
                masked={settings.storage_credentials.masked_access_key_id}
                accessKey={accessKey}
                secretKey={secretKey}
                setAccessKey={setAccessKey}
                setSecretKey={setSecretKey}
              />
            )}
          </FieldGroup>
        </CardContent>
        <CardFooter className="justify-end border-t">
          <Button
            disabled={save.isPending}
            onClick={() => (targetChanged ? setConfirming(true) : submit())}
          >
            {save.isPending ? "Testing…" : "Test and save"}
          </Button>
        </CardFooter>
      </Card>
      <ConfirmTarget
        open={confirming}
        onOpenChange={setConfirming}
        onConfirm={() => submit(true)}
      />
    </>
  );
}

function ConfirmTarget({
  open,
  onOpenChange,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Change S3 target?</AlertDialogTitle>
          <AlertDialogDescription>
            Existing objects are not moved. Confirm only after arranging
            migration or accepting that old files may be unavailable.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>
            Change target
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

export function BackupSettingsPanel() {
  const query = useSettings();
  if (query.isLoading) return <SettingsLoading />;
  if (!query.data)
    return <p className="text-sm text-destructive">Unable to load settings.</p>;
  return (
    <BackupForm
      key={JSON.stringify(query.data.backups)}
      settings={query.data}
    />
  );
}

function BackupForm({ settings }: { settings: InstanceSettings }) {
  const [form, setForm] = useState(settings.backups);
  const [accessKey, setAccessKey] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [confirming, setConfirming] = useState(false);
  const save = useSave("backups");
  const targetChanged =
    (settings.backups.destination === "s3" || form.destination === "s3") &&
    (settings.backups.destination !== form.destination ||
      settings.backups.bucket !== form.bucket ||
      settings.backups.region !== form.region ||
      settings.backups.endpoint !== form.endpoint);
  const submit = (confirmed = false) =>
    save.mutate(
      {
        ...form,
        access_key_id: accessKey || null,
        secret_access_key: secretKey || null,
        confirm_target_change: confirmed,
      },
      {
        onSuccess: () => {
          setAccessKey("");
          setSecretKey("");
        },
      },
    );
  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Backup destination</CardTitle>
          <CardDescription>
            Scheduled backup policy and its separate destination.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <FieldGroup>
            <ToggleField
              label="Scheduled backups"
              description="Run configured backup schedules."
              checked={form.enabled}
              onCheckedChange={(value) => setForm({ ...form, enabled: value })}
            />
            <TextField
              label="Default retention"
              type="number"
              value={String(form.retention)}
              onChange={(value) =>
                setForm({ ...form, retention: Number(value) })
              }
            />
            <ProviderFields
              section={form}
              onChange={(next) => setForm(next as InstanceSettings["backups"])}
            />
            {form.destination === "s3" && (
              <Credentials
                configured={settings.backup_credentials.configured}
                masked={settings.backup_credentials.masked_access_key_id}
                accessKey={accessKey}
                secretKey={secretKey}
                setAccessKey={setAccessKey}
                setSecretKey={setSecretKey}
              />
            )}
          </FieldGroup>
        </CardContent>
        <CardFooter className="justify-end border-t">
          <Button
            disabled={save.isPending}
            onClick={() => (targetChanged ? setConfirming(true) : submit())}
          >
            {save.isPending ? "Testing…" : "Save destination"}
          </Button>
        </CardFooter>
      </Card>
      <ConfirmTarget
        open={confirming}
        onOpenChange={setConfirming}
        onConfirm={() => submit(true)}
      />
    </>
  );
}
