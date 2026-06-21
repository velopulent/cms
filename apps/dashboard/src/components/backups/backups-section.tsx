import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Database,
  Download,
  Lock,
  Play,
  Plus,
  RotateCcw,
  Trash2,
} from "lucide-react";
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
import { Field, FieldLabel } from "@/components/ui/field";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  type BackupInfo,
  type BackupScope,
  type BackupSiteRef,
  backupDownloadUrl,
  backupScopeKey,
  createBackup,
  createBackupSchedule,
  deleteBackup,
  deleteBackupSchedule,
  type InspectResult,
  inspectBackup,
  inspectBackupUpload,
  listBackupSchedules,
  listBackups,
  restoreBackup,
  restoreBackupUpload,
  runBackupSchedule,
  updateBackupSchedule,
} from "@/lib/api";

const CRON_PRESETS = [
  { label: "Daily at 02:00", value: "0 2 * * *" },
  { label: "Weekly (Sunday 03:00)", value: "0 3 * * 0" },
  { label: "Monthly (1st, 04:00)", value: "0 4 1 * *" },
  { label: "Custom…", value: "custom" },
];

const RESTORE_WORD = "RESTORE";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value.toFixed(1)} ${units[i]}`;
}

function statusVariant(status: string): "default" | "secondary" | "destructive" {
  if (status === "success") return "default";
  if (status === "failed") return "destructive";
  return "secondary";
}

type RestoreSource = { type: "backup"; backup: BackupInfo } | { type: "upload"; file: File };

export function BackupsSection({ scope }: { scope: BackupScope }) {
  const queryClient = useQueryClient();
  const scopeKey = backupScopeKey(scope);
  const isInstance = scope.kind === "instance";

  const [includeFiles, setIncludeFiles] = useState(true);
  const [encrypt, setEncrypt] = useState(false);

  const [restoreSource, setRestoreSource] = useState<RestoreSource | null>(null);
  const [restoreMode, setRestoreMode] = useState<"instance" | "site">(isInstance ? "instance" : "site");
  const [selectedSiteIds, setSelectedSiteIds] = useState<string[]>([]);
  const [importAsNew, setImportAsNew] = useState(false);
  const [confirmText, setConfirmText] = useState("");
  // Sites contained in the chosen backup (instance scope only); null while loading.
  const [inspect, setInspect] = useState<InspectResult | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [inspectError, setInspectError] = useState<string | null>(null);

  const backupsQuery = useQuery({
    queryKey: ["backups", scopeKey],
    queryFn: () => listBackups(scope),
  });
  const schedulesQuery = useQuery({
    queryKey: ["backup-schedules", scopeKey],
    queryFn: () => listBackupSchedules(scope),
  });

  const invalidateBackups = () =>
    queryClient.invalidateQueries({ queryKey: ["backups", scopeKey] });
  const invalidateSchedules = () =>
    queryClient.invalidateQueries({ queryKey: ["backup-schedules", scopeKey] });

  const createMutation = useMutation({
    mutationFn: () => createBackup(scope, { include_files: includeFiles, encrypt }),
    onSuccess: () => {
      invalidateBackups();
      toast.success("Backup created");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteBackup(scope, id),
    onSuccess: () => {
      invalidateBackups();
      toast.success("Backup deleted");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  const restoreMutation = useMutation({
    mutationFn: async () => {
      if (!restoreSource) return;
      // Site-settings scope: always restores into the current site (id from URL).
      if (!isInstance) {
        const opts = { mode: "site" as const, import_as_new: importAsNew, confirm: RESTORE_WORD };
        if (restoreSource.type === "upload") {
          await restoreBackupUpload(scope, restoreSource.file, opts);
        } else {
          await restoreBackup(scope, { backup_id: restoreSource.backup.id, ...opts });
        }
        return;
      }

      // Instance scope: a site backup restores its single site; an instance backup
      // restores either the whole instance or the selected sites.
      const singleSite = inspect?.scope === "site";
      const mode: "instance" | "site" = singleSite ? "site" : restoreMode;
      const site_ids = singleSite
        ? inspect?.sites.map((s) => s.id)
        : restoreMode === "site"
          ? selectedSiteIds
          : undefined;
      const input = {
        mode,
        site_ids,
        ...(mode === "site" && { import_as_new: importAsNew }),
        confirm: RESTORE_WORD,
        // Uploads were staged during inspect — restore by key, no re-upload.
        ...(restoreSource.type === "upload"
          ? { destination_key: inspect?.staging_key ?? undefined }
          : { backup_id: restoreSource.backup.id }),
      };
      await restoreBackup(scope, input);
    },
    onSuccess: () => {
      closeRestore();
      invalidateBackups();
      queryClient.invalidateQueries();
      toast.success("Restore complete");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  async function openRestore(source: RestoreSource) {
    setRestoreSource(source);
    setRestoreMode(isInstance ? "instance" : "site");
    setSelectedSiteIds([]);
    setImportAsNew(false);
    setConfirmText("");
    setInspect(null);
    setInspectError(null);
    // Only instance restores need to know which sites a backup contains.
    if (!isInstance) return;
    setInspecting(true);
    try {
      const result =
        source.type === "upload"
          ? await inspectBackupUpload(scope, source.file)
          : await inspectBackup(scope, { backup_id: source.backup.id });
      setInspect(result);
      // A single-site backup forces "site" mode; an instance backup keeps the picker.
      if (result.scope === "site") setRestoreMode("site");
    } catch (e) {
      setInspectError((e as Error).message);
    } finally {
      setInspecting(false);
    }
  }
  function closeRestore() {
    setRestoreSource(null);
  }

  function toggleSite(id: string) {
    setSelectedSiteIds((prev) => (prev.includes(id) ? prev.filter((s) => s !== id) : [...prev, id]));
  }

  const backups = backupsQuery.data ?? [];
  const schedules = schedulesQuery.data ?? [];
  // For an instance backup picking sites, at least one site must be selected.
  const needsSitePick = isInstance && inspect?.scope === "instance" && restoreMode === "site";
  const confirmReady =
    confirmText === RESTORE_WORD &&
    !inspecting &&
    (!isInstance || (!inspectError && inspect !== null)) &&
    (!needsSitePick || selectedSiteIds.length > 0);

  return (
    <div className="flex flex-col gap-6">
      {/* Create backup */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="size-5" /> Backups
          </CardTitle>
          <CardDescription>
            {isInstance
              ? "Capture the whole instance — every site plus users and roles."
              : "Capture this site's content, schema, files, and settings."}{" "}
            Backups are compressed and stored in the configured destination.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-wrap items-center gap-4">
              <Field orientation="horizontal">
                <Checkbox
                  id="backup-include-files"
                  checked={includeFiles}
                  onCheckedChange={(v) => setIncludeFiles(Boolean(v))}
                />
                <FieldLabel htmlFor="backup-include-files">Include uploaded files</FieldLabel>
              </Field>
              <Field orientation="horizontal">
                <Checkbox id="backup-encrypt" checked={encrypt} onCheckedChange={(v) => setEncrypt(Boolean(v))} />
                <FieldLabel htmlFor="backup-encrypt">Encrypt</FieldLabel>
              </Field>
            </div>
            <div className="flex gap-2">
              <label>
                <input
                  type="file"
                  className="hidden"
                  accept=".cmsbak"
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (file) openRestore({ type: "upload", file });
                    e.target.value = "";
                  }}
                />
                <Button variant="outline" render={<span />}>
                  <RotateCcw className="size-4" /> Restore from file
                </Button>
              </label>
              <Button onClick={() => createMutation.mutate()} disabled={createMutation.isPending}>
                <Plus className="size-4" />
                {createMutation.isPending ? "Backing up…" : "Back up now"}
              </Button>
            </div>
          </div>

          {backupsQuery.isLoading ? (
            <Skeleton className="h-24 w-full" />
          ) : backups.length === 0 ? (
            <p className="text-sm text-muted-foreground">No backups yet.</p>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Created</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Size</TableHead>
                    <TableHead className="hidden sm:table-cell">Files</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {backups.map((b) => (
                    <TableRow key={b.id}>
                      <TableCell className="whitespace-nowrap">
                        {new Date(b.created_at).toLocaleString()}
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-1.5">
                          <Badge variant={statusVariant(b.status)}>{b.status}</Badge>
                          {b.encrypted && <Lock className="size-3.5 text-muted-foreground" />}
                        </div>
                      </TableCell>
                      <TableCell className="whitespace-nowrap">{formatBytes(b.size_bytes)}</TableCell>
                      <TableCell className="hidden sm:table-cell">
                        {b.includes_files ? b.file_count : "—"}
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center justify-end gap-1">
                          {b.status === "success" && (
                            <>
                              <Button
                                variant="ghost"
                                size="icon"
                                title="Download"
                                render={<a href={backupDownloadUrl(scope, b.id)}><Download className="size-4" /></a>}
                              />
                              <Button
                                variant="ghost"
                                size="icon"
                                title="Restore"
                                onClick={() => openRestore({ type: "backup", backup: b })}
                              >
                                <RotateCcw className="size-4" />
                              </Button>
                            </>
                          )}
                          <Button
                            variant="ghost"
                            size="icon"
                            title="Delete"
                            onClick={() => deleteMutation.mutate(b.id)}
                            disabled={deleteMutation.isPending}
                          >
                            <Trash2 className="size-4" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>

      <SchedulesCard
        schedules={schedules}
        loading={schedulesQuery.isLoading}
        onCreate={async (input) => {
          await createBackupSchedule(scope, input);
          invalidateSchedules();
        }}
        onToggle={async (s) => {
          await updateBackupSchedule(scope, s.id, {
            cron: s.cron,
            retention_n: s.retention_n,
            include_files: s.include_files,
            encrypt: s.encrypt,
            enabled: !s.enabled,
          });
          invalidateSchedules();
        }}
        onRun={async (id) => {
          await runBackupSchedule(scope, id);
          invalidateBackups();
          toast.success("Backup started");
        }}
        onDelete={async (id) => {
          await deleteBackupSchedule(scope, id);
          invalidateSchedules();
        }}
      />

      {/* Restore confirmation */}
      <Dialog open={!!restoreSource} onOpenChange={(open) => !open && closeRestore()}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-destructive">
              <AlertTriangle className="size-5" /> Confirm restore
            </DialogTitle>
            <DialogDescription>
              Restoring replaces all data within the chosen scope. This cannot be undone.
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4">
            {isInstance && inspecting && (
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Skeleton className="h-9 w-full" />
              </div>
            )}
            {isInstance && inspectError && (
              <p className="text-sm text-destructive">Could not read this backup: {inspectError}</p>
            )}

            {/* A single-site backup: no picker, just name the site being restored. */}
            {isInstance && inspect?.scope === "site" && (
              <p className="text-sm text-muted-foreground">
                Restores the site{" "}
                <span className="font-medium text-foreground">
                  {inspect.sites[0]?.name ?? inspect.sites[0]?.id ?? "in this backup"}
                </span>
                .
              </p>
            )}

            {/* An instance backup: choose whole instance or specific sites. */}
            {isInstance && inspect?.scope === "instance" && (
              <div className="flex flex-col gap-2">
                <Label>What to restore</Label>
                <Select value={restoreMode} onValueChange={(v) => setRestoreMode(v as "instance" | "site")}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="instance">Whole instance</SelectItem>
                    <SelectItem value="site">Selected sites</SelectItem>
                  </SelectContent>
                </Select>
                {restoreMode === "site" && (
                  <SitePicker
                    sites={inspect.sites}
                    selected={selectedSiteIds}
                    onToggle={toggleSite}
                    onToggleAll={(all) => setSelectedSiteIds(all ? inspect.sites.map((s) => s.id) : [])}
                  />
                )}
              </div>
            )}

            {((isInstance && restoreMode === "site") || !isInstance) && (
              <Field orientation="horizontal">
                <Checkbox id="restore-import-as-new" checked={importAsNew} onCheckedChange={(v) => setImportAsNew(Boolean(v))} />
                <FieldLabel htmlFor="restore-import-as-new">Import as a new site (keep the existing one)</FieldLabel>
              </Field>
            )}

            <div className="flex flex-col gap-2">
              <Label htmlFor="restore-confirm">
                Type <span className="font-mono font-semibold">{RESTORE_WORD}</span> to confirm
              </Label>
              <Input
                id="restore-confirm"
                value={confirmText}
                onChange={(e) => setConfirmText(e.target.value)}
                autoComplete="off"
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={closeRestore}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              disabled={!confirmReady || restoreMutation.isPending}
              onClick={() => restoreMutation.mutate()}
            >
              {restoreMutation.isPending ? "Restoring…" : "Restore"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/** Multi-select list of the sites contained in a backup. */
function SitePicker({
  sites,
  selected,
  onToggle,
  onToggleAll,
}: {
  sites: BackupSiteRef[];
  selected: string[];
  onToggle: (id: string) => void;
  onToggleAll: (all: boolean) => void;
}) {
  if (sites.length === 0) {
    return <p className="text-sm text-muted-foreground">This backup contains no sites.</p>;
  }
  const allSelected = selected.length === sites.length;
  return (
    <div className="rounded-md border">
      <Field orientation="horizontal" className="border-b px-3 py-2 text-sm font-medium">
        <Checkbox id="site-picker-select-all" checked={allSelected} onCheckedChange={(v) => onToggleAll(Boolean(v))} />
        <FieldLabel htmlFor="site-picker-select-all">Select all ({selected.length}/{sites.length})</FieldLabel>
      </Field>
      <ScrollArea className="max-h-48">
        <div className="flex flex-col">
          {sites.map((s) => (
            <Field key={s.id} orientation="horizontal" className="px-3 py-2 text-sm hover:bg-muted/50">
              <Checkbox id={`site-picker-${s.id}`} checked={selected.includes(s.id)} onCheckedChange={() => onToggle(s.id)} />
              <FieldLabel htmlFor={`site-picker-${s.id}`}>
                <span className="flex flex-col">
                  <span className="font-medium">{s.name ?? "(unnamed site)"}</span>
                  <span className="font-mono text-xs text-muted-foreground">{s.id}</span>
                </span>
              </FieldLabel>
            </Field>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}

interface SchedulesCardProps {
  schedules: import("@/lib/api").BackupSchedule[];
  loading: boolean;
  onCreate: (input: import("@/lib/api").ScheduleInput) => Promise<void>;
  onToggle: (s: import("@/lib/api").BackupSchedule) => Promise<void>;
  onRun: (id: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}

function SchedulesCard({ schedules, loading, onCreate, onToggle, onRun, onDelete }: SchedulesCardProps) {
  const [preset, setPreset] = useState(CRON_PRESETS[0].value);
  const [customCron, setCustomCron] = useState("0 2 * * *");
  const [retention, setRetention] = useState(7);
  const [includeFiles, setIncludeFiles] = useState(true);
  const [encrypt, setEncrypt] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const cron = preset === "custom" ? customCron : preset;

  async function submit() {
    setSubmitting(true);
    try {
      await onCreate({
        cron,
        retention_n: Math.max(1, retention),
        include_files: includeFiles,
        encrypt,
        enabled: true,
      });
      toast.success("Schedule added");
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Scheduled backups</CardTitle>
        <CardDescription>
          Run backups automatically on a schedule and keep the most recent N.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 lg:items-end">
          <div className="flex flex-col gap-1.5">
            <Label>Frequency</Label>
            <Select value={preset} onValueChange={(v) => setPreset(v ?? "")}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {CRON_PRESETS.map((p) => (
                  <SelectItem key={p.value} value={p.value}>
                    {p.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {preset === "custom" && (
              <Input
                className="mt-1 font-mono"
                value={customCron}
                onChange={(e) => setCustomCron(e.target.value)}
                placeholder="min hour dom mon dow"
              />
            )}
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="retention">Keep last N</Label>
            <Input
              id="retention"
              type="number"
              min={1}
              value={retention}
              onChange={(e) => setRetention(Number(e.target.value))}
            />
          </div>
          <div className="flex flex-col gap-2 sm:col-span-2 lg:col-span-1 lg:flex-row lg:items-center lg:gap-4 lg:pb-2">
            <Field orientation="horizontal">
              <Checkbox id="schedule-files" checked={includeFiles} onCheckedChange={(v) => setIncludeFiles(Boolean(v))} />
              <FieldLabel htmlFor="schedule-files">Files</FieldLabel>
            </Field>
            <Field orientation="horizontal">
              <Checkbox id="schedule-encrypt" checked={encrypt} onCheckedChange={(v) => setEncrypt(Boolean(v))} />
              <FieldLabel htmlFor="schedule-encrypt">Encrypt</FieldLabel>
            </Field>
          </div>
          <Button onClick={submit} disabled={submitting}>
            <Plus className="size-4" /> Add schedule
          </Button>
        </div>

        {loading ? (
          <Skeleton className="h-16 w-full" />
        ) : schedules.length === 0 ? (
          <p className="text-sm text-muted-foreground">No schedules configured.</p>
        ) : (
          <div className="flex flex-col gap-2">
            {schedules.map((s) => (
              <div
                key={s.id}
                className="flex flex-col gap-2 rounded-md border p-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div className="flex flex-col gap-0.5">
                  <div className="flex items-center gap-2">
                    <code className="font-mono text-sm">{s.cron}</code>
                    <Badge variant={s.enabled ? "default" : "secondary"}>
                      {s.enabled ? "Enabled" : "Paused"}
                    </Badge>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Keep {s.retention_n} · {s.include_files ? "with files" : "no files"} ·{" "}
                    {s.encrypt ? "encrypted" : "plaintext"}
                    {s.next_run_at && ` · next ${new Date(s.next_run_at).toLocaleString()}`}
                  </p>
                </div>
                <div className="flex items-center gap-1">
                  <Button variant="ghost" size="icon" title="Run now" onClick={() => onRun(s.id)}>
                    <Play className="size-4" />
                  </Button>
                  <Button variant="ghost" size="sm" onClick={() => onToggle(s)}>
                    {s.enabled ? "Pause" : "Resume"}
                  </Button>
                  <Button variant="ghost" size="icon" title="Delete" onClick={() => onDelete(s.id)}>
                    <Trash2 className="size-4" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
