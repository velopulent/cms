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
  backupDownloadUrl,
  backupScopeKey,
  createBackup,
  createBackupSchedule,
  deleteBackup,
  deleteBackupSchedule,
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
  const [restoreSiteId, setRestoreSiteId] = useState("");
  const [importAsNew, setImportAsNew] = useState(false);
  const [confirmText, setConfirmText] = useState("");

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
      const opts = {
        mode: isInstance ? restoreMode : ("site" as const),
        site_id: isInstance && restoreMode === "site" ? restoreSiteId : undefined,
        ...(restoreMode === "site" && { import_as_new: importAsNew }),
        confirm: RESTORE_WORD,
      };
      if (restoreSource?.type === "upload") {
        await restoreBackupUpload(scope, restoreSource.file, opts);
      } else if (restoreSource?.type === "backup") {
        await restoreBackup(scope, { backup_id: restoreSource.backup.id, ...opts });
      }
    },
    onSuccess: () => {
      closeRestore();
      invalidateBackups();
      queryClient.invalidateQueries();
      toast.success("Restore complete");
    },
    onError: (e: Error) => toast.error(e.message),
  });

  function openRestore(source: RestoreSource) {
    setRestoreSource(source);
    setRestoreMode(isInstance ? "instance" : "site");
    setRestoreSiteId("");
    setImportAsNew(false);
    setConfirmText("");
  }
  function closeRestore() {
    setRestoreSource(null);
  }

  const backups = backupsQuery.data ?? [];
  const schedules = schedulesQuery.data ?? [];
  const confirmReady =
    confirmText === RESTORE_WORD &&
    (!isInstance || restoreMode === "instance" || restoreSiteId.trim().length > 0);

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
              <label className="flex items-center gap-2 text-sm">
                <Checkbox
                  checked={includeFiles}
                  onCheckedChange={(v) => setIncludeFiles(Boolean(v))}
                />
                Include uploaded files
              </label>
              <label className="flex items-center gap-2 text-sm">
                <Checkbox checked={encrypt} onCheckedChange={(v) => setEncrypt(Boolean(v))} />
                Encrypt
              </label>
            </div>
            <div className="flex gap-2">
              <label>
                <input
                  type="file"
                  className="hidden"
                  accept=".cmsbak,application/octet-stream"
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
                                render={<a href={backupDownloadUrl(scope, b.id)} />}
                              >
                                <Download className="size-4" />
                              </Button>
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
            {isInstance && (
              <div className="flex flex-col gap-2">
                <Label>What to restore</Label>
                <Select value={restoreMode} onValueChange={(v) => setRestoreMode(v as "instance" | "site")}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="instance">Whole instance</SelectItem>
                    <SelectItem value="site">A single site</SelectItem>
                  </SelectContent>
                </Select>
                {restoreMode === "site" && (
                  <Input
                    placeholder="Site ID to restore"
                    value={restoreSiteId}
                    onChange={(e) => setRestoreSiteId(e.target.value)}
                  />
                )}
              </div>
            )}

            {(restoreMode === "site" || !isInstance) && (
              <label className="flex items-center gap-2 text-sm">
                <Checkbox checked={importAsNew} onCheckedChange={(v) => setImportAsNew(Boolean(v))} />
                Import as a new site (keep the existing one)
              </label>
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
            <label className="flex items-center gap-2 text-sm">
              <Checkbox checked={includeFiles} onCheckedChange={(v) => setIncludeFiles(Boolean(v))} />
              Files
            </label>
            <label className="flex items-center gap-2 text-sm">
              <Checkbox checked={encrypt} onCheckedChange={(v) => setEncrypt(Boolean(v))} />
              Encrypt
            </label>
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
