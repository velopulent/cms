import type { AnyFieldApi } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { Archive, Check, FileText, Music, Plus, X } from "lucide-react";
import type React from "react";
import {
  Component,
  memo,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";

import { FilePickerDialog } from "@/components/file-picker-dialog";
import { TiptapEditor } from "@/components/tiptap-editor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { VideoPlayer } from "@/components/video-player";
import {
  type ContentField,
  type Entry,
  type FileItem,
  getCollection,
  getEntries,
} from "@/lib/api";

interface DynamicFormProps {
  fields: ContentField[];
  // biome-ignore lint/suspicious/noExplicitAny: TanStack Form instance with complex generics
  form: any;
  prefix?: string;
  siteId?: string;
  readOnly?: boolean;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_PREFIX = "data";

/** Maps a file category to its accept string for the native file picker. */
const CATEGORY_ACCEPT: Record<string, string> = {
  image: "image/*",
  video: "video/*",
  audio: "audio/*",
  document: ".pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.txt,.csv,.html,.md",
  archive: ".zip,.gz,.tar,.7z,.rar",
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Normalise a field's `accept` property to `string[] | undefined`.
 * Accepts `unknown` so TypeScript's Array.isArray narrowing works correctly
 * regardless of how ContentField types the property.
 */
function normaliseAccept(accept: unknown): string[] | undefined {
  if (Array.isArray(accept)) {
    const strs = accept.filter((x): x is string => typeof x === "string");
    return strs.length > 0 ? strs : undefined;
  }
  if (typeof accept === "string" && accept.length > 0) return [accept];
  return undefined;
}

/**
 * Normalise a field's `options` property to `string[] | undefined`.
 * Same rationale as normaliseAccept.
 */
function normaliseOptions(options: unknown): string[] | undefined {
  if (Array.isArray(options)) {
    const strs = options.filter((x): x is string => typeof x === "string");
    return strs.length > 0 ? strs : undefined;
  }
  if (typeof options === "string" && options.length > 0) return [options];
  return undefined;
}

/** Convert a field.name slug into a human-readable label. */
function toLabel(name: string): string {
  return name.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Format bytes to the most appropriate unit. */
function formatFileSize(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${(bytes / 1_024).toFixed(1)} KB`;
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}

/** Extract the file UUID from an internal `/api/files/:id` URL. */
function extractFileId(url: string): string | null {
  const match = url.match(/\/api\/files\/([^/]+)/);
  return match ? match[1] : null;
}

// ---------------------------------------------------------------------------
// Per-field error boundary
// ---------------------------------------------------------------------------

interface FieldErrorBoundaryState {
  hasError: boolean;
  message: string;
}

class FieldErrorBoundary extends Component<
  React.PropsWithChildren<{ fieldName: string }>,
  FieldErrorBoundaryState
> {
  state: FieldErrorBoundaryState = { hasError: false, message: "" };

  static getDerivedStateFromError(error: unknown): FieldErrorBoundaryState {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : String(error),
    };
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          role="alert"
          className="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive"
        >
          Field "{this.props.fieldName}" failed to render: {this.state.message}
        </div>
      );
    }
    return this.props.children;
  }
}

// ---------------------------------------------------------------------------
// useFileInfo – fetches file metadata with AbortController cleanup
// ---------------------------------------------------------------------------

interface UseFileInfoResult {
  fileInfo: FileItem | null;
  isLoading: boolean;
  error: string | null;
}

function useFileInfo(
  fileId: string | null,
  siteId: string | undefined,
): UseFileInfoResult {
  const [fileInfo, setFileInfo] = useState<FileItem | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!fileId || !siteId) {
      setFileInfo(null);
      return;
    }

    const controller = new AbortController();
    setIsLoading(true);
    setError(null);

    fetch(`/api/dashboard/sites/${siteId}/files/${fileId}`, {
      credentials: "include",
      signal: controller.signal,
    })
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json() as Promise<FileItem>;
      })
      .then((data) => {
        setFileInfo(data);
        setIsLoading(false);
      })
      .catch((err: unknown) => {
        if ((err as { name?: string }).name === "AbortError") return;
        setError(err instanceof Error ? err.message : "Unknown error");
        setIsLoading(false);
      });

    return () => controller.abort();
  }, [fileId, siteId]);

  return { fileInfo, isLoading, error };
}

// ---------------------------------------------------------------------------
// DynamicForm – public entry point
// ---------------------------------------------------------------------------

export const DynamicForm = memo(function DynamicForm({
  fields,
  form,
  prefix = DEFAULT_PREFIX,
  siteId,
  readOnly,
}: DynamicFormProps) {
  return (
    <FieldGroup>
      {fields.map((field) => (
        <FieldErrorBoundary key={field.name} fieldName={field.name}>
          <DynamicField
            field={field}
            form={form}
            prefix={prefix}
            siteId={siteId}
            readOnly={readOnly}
          />
        </FieldErrorBoundary>
      ))}
    </FieldGroup>
  );
});

// ---------------------------------------------------------------------------
// DynamicField – renders a single form.Field wrapper
// ---------------------------------------------------------------------------

interface DynamicFieldProps {
  field: ContentField;
  // biome-ignore lint/suspicious/noExplicitAny: TanStack Form instance with complex generics
  form: any;
  prefix: string;
  siteId?: string;
  readOnly?: boolean;
}

const DynamicField = memo(function DynamicField({
  field,
  form,
  prefix,
  siteId,
  readOnly,
}: DynamicFieldProps) {
  const fieldName = `${prefix}.${field.name}`;
  const label = toLabel(field.name);
  const errorId = `${fieldName}-error`;

  return (
    <form.Field name={fieldName}>
      {(f: AnyFieldApi) => {
        const isInvalid = f.state.meta.isTouched && !f.state.meta.isValid;
        return (
          <Field data-invalid={isInvalid || undefined}>
            <FieldLabel htmlFor={fieldName}>
              {label}
              {field.required && (
                <span className="ml-1 text-destructive" aria-hidden="true">
                  *
                </span>
              )}
            </FieldLabel>
            <FieldInput
              field={field}
              value={f.state.value}
              onChange={readOnly ? noop : f.handleChange}
              onBlur={readOnly ? noop : f.handleBlur}
              isInvalid={isInvalid}
              siteId={siteId}
              fieldName={fieldName}
              errorId={errorId}
              readOnly={readOnly}
            />
            {isInvalid && (
              <FieldError id={errorId} errors={f.state.meta.errors} />
            )}
          </Field>
        );
      }}
    </form.Field>
  );
});

// Stable no-op to avoid new function refs on every render in readOnly mode
function noop() {}

// ---------------------------------------------------------------------------
// FieldInput – routes to the appropriate input widget
// ---------------------------------------------------------------------------

interface FieldInputProps {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
  onBlur: () => void;
  isInvalid: boolean;
  siteId?: string;
  fieldName: string;
  errorId: string;
  readOnly?: boolean;
}

const FieldInput = memo(function FieldInput({
  field,
  value,
  onChange,
  onBlur,
  isInvalid,
  siteId,
  fieldName,
  errorId,
  readOnly,
}: FieldInputProps) {
  const strValue = typeof value === "string" ? value : "";
  const numValue = typeof value === "number" ? value : 0;
  const boolValue = typeof value === "boolean" ? value : false;

  const accept = useMemo(() => normaliseAccept(field.accept), [field.accept]);
  const options = useMemo(
    () => normaliseOptions(field.options),
    [field.options],
  );

  /** Shared props for simple <Input> fields. */
  const inputBaseProps = {
    id: fieldName,
    "aria-invalid": isInvalid || undefined,
    "aria-required": field.required || undefined,
    "aria-errormessage": isInvalid ? errorId : undefined,
    readOnly,
    disabled: readOnly,
    onBlur,
  } as const;

  const stableStringChange = useStableHandler(onChange, extractString);
  const stableNumberChange = useStableHandler(onChange, extractNumber);

  switch (field.type) {
    case "text":
      return (
        <Input
          {...inputBaseProps}
          value={strValue}
          onChange={stableStringChange}
        />
      );

    case "textarea":
      return (
        <Textarea
          {...inputBaseProps}
          value={strValue}
          rows={4}
          onChange={stableStringChange}
        />
      );

    case "rich_text":
      return (
        <TiptapEditor
          content={strValue}
          onChange={onChange}
          placeholder={`Write ${field.name}…`}
          siteId={siteId}
          editable={!readOnly}
        />
      );

    case "number":
      return (
        <Input
          {...inputBaseProps}
          type="number"
          min={field.min}
          max={field.max}
          value={numValue}
          onChange={stableNumberChange}
        />
      );

    case "email":
      return (
        <Input
          {...inputBaseProps}
          type="email"
          placeholder="name@example.com"
          value={strValue}
          onChange={stableStringChange}
        />
      );

    case "url":
      return (
        <Input
          {...inputBaseProps}
          type="url"
          placeholder="https://…"
          value={strValue}
          onChange={stableStringChange}
        />
      );

    case "json":
      return (
        <JsonField
          value={value}
          onChange={onChange}
          onBlur={onBlur}
          isInvalid={isInvalid}
          fieldName={fieldName}
          errorId={errorId}
          readOnly={readOnly}
        />
      );

    case "relation":
      return (
        <RelationField
          value={value}
          onChange={onChange}
          siteId={siteId}
          targetSlug={field.target_collection}
          multiple={field.multiple}
          maxSelect={field.max_select}
          readOnly={readOnly}
        />
      );

    case "boolean":
      return (
        <Field orientation="horizontal">
          <Checkbox
            id={fieldName}
            checked={boolValue}
            onCheckedChange={(checked) => onChange(!!checked)}
            aria-invalid={isInvalid || undefined}
            aria-required={field.required || undefined}
            aria-errormessage={isInvalid ? errorId : undefined}
            disabled={readOnly}
          />
          <FieldLabel htmlFor={fieldName} className="font-normal">
            Enabled
          </FieldLabel>
        </Field>
      );

    case "date":
      return (
        <Input
          {...inputBaseProps}
          type="date"
          value={strValue}
          onChange={stableStringChange}
        />
      );

    case "select":
      if (field.multiple) {
        return (
          <MultiSelectField
            value={value}
            onChange={onChange}
            options={options ?? []}
            maxSelect={field.max_select}
            readOnly={readOnly}
          />
        );
      }
      return (
        <Select value={strValue} onValueChange={onChange} disabled={readOnly}>
          <SelectTrigger
            id={fieldName}
            aria-invalid={isInvalid || undefined}
            aria-required={field.required || undefined}
            aria-errormessage={isInvalid ? errorId : undefined}
            className="w-full"
          >
            <SelectValue placeholder={`Select ${field.name}`} />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {(options ?? []).map((opt) => (
                <SelectItem key={opt} value={opt}>
                  {opt}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
      );

    case "image_url":
      return (
        <div className="flex flex-col gap-2">
          <Input
            {...inputBaseProps}
            placeholder="https://…"
            value={strValue}
            onChange={stableStringChange}
          />
          {strValue && (
            <img
              src={strValue}
              alt="Preview"
              className="h-32 w-auto rounded-lg border object-cover"
              loading="lazy"
            />
          )}
        </div>
      );

    case "image":
    case "video":
    case "audio":
    case "document":
    case "archive":
      if (field.multiple) {
        return (
          <MultiFileField
            value={value}
            onChange={onChange}
            siteId={siteId}
            readOnly={readOnly}
            category={field.type}
            accept={accept}
            maxSelect={field.max_select}
          />
        );
      }
      return (
        <FileField
          value={strValue}
          onChange={onChange}
          siteId={siteId}
          isInvalid={isInvalid}
          errorId={errorId}
          required={field.required}
          readOnly={readOnly}
          category={field.type}
          accept={accept}
        />
      );

    default:
      return (
        <Input
          {...inputBaseProps}
          value={strValue}
          onChange={stableStringChange}
        />
      );
  }
});

// ---------------------------------------------------------------------------
// Stable input-event extractors (avoid inline arrow fns in render)
// ---------------------------------------------------------------------------

function extractString(
  e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>,
) {
  return e.target.value;
}

function extractNumber(e: React.ChangeEvent<HTMLInputElement>) {
  return Number(e.target.value) || 0;
}

/**
 * Returns a stable memoised event handler that calls onChange with the value
 * extracted by the provided extractor function.
 *
 * Note: This is called at component render time but always with the same
 * extractor reference, so the identity of the returned handler is stable as
 * long as `onChange` is stable (guaranteed by TanStack Form).
 */
function useStableHandler<
  E extends React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>,
  T,
>(onChange: (val: T) => void, extract: (e: E) => T): (e: E) => void {
  return useCallback((e: E) => onChange(extract(e)), [onChange, extract]);
}

// ---------------------------------------------------------------------------
// FileField – file picker + preview
// ---------------------------------------------------------------------------

interface FileFieldProps {
  value: string;
  onChange: (val: unknown) => void;
  siteId?: string;
  isInvalid: boolean;
  errorId: string;
  required?: boolean;
  readOnly?: boolean;
  category?: string;
  accept?: string[];
}

const FileField = memo(function FileField({
  value,
  onChange,
  siteId,
  isInvalid,
  errorId,
  required,
  readOnly,
  category,
  accept,
}: FileFieldProps) {
  const [pickerOpen, setPickerOpen] = useState(false);

  const fileId = useMemo(() => extractFileId(value), [value]);
  const isExternalUrl = !fileId && value.startsWith("http");

  const { fileInfo, isLoading } = useFileInfo(fileId, siteId);

  const filterAccept = useMemo(
    () =>
      accept?.join(", ") ?? (category ? CATEGORY_ACCEPT[category] : undefined),
    [accept, category],
  );

  const handleSelect = useCallback(
    (file: FileItem) => {
      onChange(file.url);
    },
    [onChange],
  );

  const handleRemove = useCallback(() => {
    onChange("");
  }, [onChange]);

  const handleOpenPicker = useCallback(() => setPickerOpen(true), []);

  return (
    <div className="flex flex-col gap-2">
      {value && (
        <FilePreview
          value={value}
          fileId={fileId}
          isExternalUrl={isExternalUrl}
          fileInfo={fileInfo}
          isLoading={isLoading}
          readOnly={readOnly}
          onRemove={handleRemove}
        />
      )}

      {!readOnly && (
        <Button
          type="button"
          variant="outline"
          onClick={handleOpenPicker}
          disabled={!siteId}
          aria-invalid={isInvalid || undefined}
          aria-required={required || undefined}
          aria-errormessage={isInvalid ? errorId : undefined}
        >
          {value ? "Change File" : "Select File"}
        </Button>
      )}

      {siteId && (
        <FilePickerDialog
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          onSelect={handleSelect}
          siteId={siteId}
          accept={filterAccept}
        />
      )}
    </div>
  );
});

// ---------------------------------------------------------------------------
// FilePreview – displays the currently selected file
// ---------------------------------------------------------------------------

interface FilePreviewProps {
  value: string;
  fileId: string | null;
  isExternalUrl: boolean;
  fileInfo: FileItem | null;
  isLoading: boolean;
  readOnly?: boolean;
  onRemove: () => void;
}

const FilePreview = memo(function FilePreview({
  value,
  fileId,
  isExternalUrl,
  fileInfo,
  isLoading,
  readOnly,
  onRemove,
}: FilePreviewProps) {
  const mime = fileInfo?.mime_type ?? "";
  const isVideo = mime.startsWith("video/");
  const isAudio = mime.startsWith("audio/");
  const isImage = mime.startsWith("image/") || (!fileInfo && isExternalUrl);
  const isDocument =
    mime.startsWith("application/pdf") ||
    mime.startsWith("application/msword") ||
    mime.startsWith("application/vnd.") ||
    mime.startsWith("text/");

  const thumbnailSrc = isVideo
    ? (fileInfo?.thumbnail_url ?? null)
    : fileId && isImage
      ? `/api/files/${fileId}/thumbnail`
      : isExternalUrl && isImage
        ? value
        : null;

  return (
    <div className="relative flex flex-col gap-3 rounded-lg border p-2">
      <div className="flex items-center gap-3">
        {/* Thumbnail / icon */}
        {thumbnailSrc ? (
          <img
            src={thumbnailSrc}
            alt=""
            aria-hidden="true"
            className="h-16 w-16 rounded object-cover"
            loading="lazy"
            onError={(e) => {
              (e.currentTarget as HTMLImageElement).style.display = "none";
            }}
          />
        ) : isAudio ? (
          <FileTypeIcon
            icon={<Music className="size-6 text-muted-foreground" />}
          />
        ) : isDocument ? (
          <FileTypeIcon
            icon={<FileText className="size-6 text-muted-foreground" />}
          />
        ) : (
          <FileTypeIcon
            icon={<Archive className="size-6 text-muted-foreground" />}
          />
        )}

        {/* File metadata */}
        <div className="min-w-0 flex-1">
          <Badge variant="secondary" className="max-w-full truncate text-xs">
            {isLoading
              ? "Loading…"
              : (fileInfo?.original_name ??
                (fileId ? `${fileId.slice(0, 8)}…` : "File selected"))}
          </Badge>
          {mime && (
            <p className="mt-1 truncate text-xs text-muted-foreground">
              {mime}
              {fileInfo?.size != null
                ? ` — ${formatFileSize(fileInfo.size)}`
                : ""}
            </p>
          )}
        </div>

        {!readOnly && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onRemove}
            aria-label="Remove selected file"
          >
            Remove
          </Button>
        )}
      </div>

      {/* Media players */}
      {isVideo && value && (
        <VideoPlayer
          src={value}
          poster={fileInfo?.thumbnail_url ?? undefined}
          className="w-full overflow-hidden rounded"
        />
      )}
      {isAudio && value && (
        <audio controls src={value} className="w-full">
          <track kind="captions" />
        </audio>
      )}
    </div>
  );
});

// ---------------------------------------------------------------------------
// FileTypeIcon – small helper for the file-type icon box
// ---------------------------------------------------------------------------

const FileTypeIcon = memo(function FileTypeIcon({
  icon,
}: {
  icon: React.ReactNode;
}) {
  return (
    <div
      className="flex h-16 w-16 shrink-0 items-center justify-center rounded bg-muted"
      aria-hidden="true"
    >
      {icon}
    </div>
  );
});

// ---------------------------------------------------------------------------
// JsonField – textarea with live JSON parse feedback
// ---------------------------------------------------------------------------

function jsonToText(value: unknown): string {
  if (value === undefined || value === null || value === "") return "";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "";
  }
}

const JsonField = memo(function JsonField({
  value,
  onChange,
  onBlur,
  isInvalid,
  fieldName,
  errorId,
  readOnly,
}: {
  value: unknown;
  onChange: (val: unknown) => void;
  onBlur: () => void;
  isInvalid: boolean;
  fieldName: string;
  errorId: string;
  readOnly?: boolean;
}) {
  const [text, setText] = useState(() => jsonToText(value));
  const [parseError, setParseError] = useState<string | null>(null);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const t = e.target.value;
      setText(t);
      if (t.trim() === "") {
        setParseError(null);
        onChange(null);
        return;
      }
      try {
        const parsed = JSON.parse(t);
        setParseError(null);
        onChange(parsed);
      } catch (err) {
        setParseError(err instanceof Error ? err.message : "Invalid JSON");
      }
    },
    [onChange],
  );

  return (
    <div className="flex flex-col gap-1">
      <Textarea
        id={fieldName}
        value={text}
        onChange={handleChange}
        onBlur={onBlur}
        rows={6}
        readOnly={readOnly}
        disabled={readOnly}
        aria-invalid={isInvalid || !!parseError || undefined}
        aria-errormessage={isInvalid ? errorId : undefined}
        className="font-mono text-xs"
        placeholder={'{ "key": "value" }'}
        spellCheck={false}
      />
      {parseError && (
        <p className="text-xs text-destructive">Invalid JSON: {parseError}</p>
      )}
    </div>
  );
});

// ---------------------------------------------------------------------------
// MultiSelectField – toggle chips for `select` with multiple = true
// ---------------------------------------------------------------------------

const MultiSelectField = memo(function MultiSelectField({
  value,
  onChange,
  options,
  maxSelect,
  readOnly,
}: {
  value: unknown;
  onChange: (val: unknown) => void;
  options: string[];
  maxSelect?: number;
  readOnly?: boolean;
}) {
  const selected = Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];

  const toggle = (opt: string) => {
    if (readOnly) return;
    if (selected.includes(opt)) {
      onChange(selected.filter((s) => s !== opt));
    } else {
      if (maxSelect && selected.length >= maxSelect) return;
      onChange([...selected, opt]);
    }
  };

  return (
    <div className="flex flex-wrap gap-1">
      {options.length === 0 && (
        <p className="text-xs text-muted-foreground">No options defined.</p>
      )}
      {options.map((opt) => {
        const isSel = selected.includes(opt);
        return (
          <button
            key={opt}
            type="button"
            disabled={readOnly}
            onClick={() => toggle(opt)}
            className={`inline-flex items-center gap-1 rounded-md border px-2 py-1 text-xs transition-colors ${
              isSel
                ? "border-primary bg-primary/10 text-primary"
                : "border-border bg-background text-muted-foreground hover:text-foreground"
            }`}
          >
            {isSel && <Check className="size-3" />}
            {opt}
          </button>
        );
      })}
    </div>
  );
});

// ---------------------------------------------------------------------------
// MultiFileField – array of file URLs via the file picker
// ---------------------------------------------------------------------------

const MultiFileField = memo(function MultiFileField({
  value,
  onChange,
  siteId,
  readOnly,
  category,
  accept,
  maxSelect,
}: {
  value: unknown;
  onChange: (val: unknown) => void;
  siteId?: string;
  readOnly?: boolean;
  category?: string;
  accept?: string[];
  maxSelect?: number;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const urls = Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];

  const filterAccept = useMemo(
    () =>
      accept?.join(", ") ?? (category ? CATEGORY_ACCEPT[category] : undefined),
    [accept, category],
  );

  const handleSelect = useCallback(
    (file: FileItem) => {
      if (!urls.includes(file.url)) onChange([...urls, file.url]);
    },
    [urls, onChange],
  );

  const removeAt = (idx: number) => onChange(urls.filter((_, i) => i !== idx));

  const limitReached = maxSelect != null && urls.length >= maxSelect;

  return (
    <div className="flex flex-col gap-2">
      {urls.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {urls.map((url, idx) => (
            <span
              key={url}
              className="inline-flex max-w-full items-center gap-1 rounded-md border bg-background px-2 py-1 text-xs"
            >
              <span className="truncate">{url.split("/").pop() ?? url}</span>
              {!readOnly && (
                <button
                  type="button"
                  className="text-muted-foreground hover:text-foreground"
                  onClick={() => removeAt(idx)}
                  aria-label="Remove file"
                >
                  <X className="size-3" />
                </button>
              )}
            </span>
          ))}
        </div>
      )}
      {!readOnly && (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="w-fit"
          disabled={!siteId || limitReached}
          onClick={() => setPickerOpen(true)}
        >
          <Plus data-icon="inline-start" />
          {limitReached ? `Max ${maxSelect} reached` : "Add file"}
        </Button>
      )}
      {siteId && (
        <FilePickerDialog
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          onSelect={handleSelect}
          siteId={siteId}
          accept={filterAccept}
        />
      )}
    </div>
  );
});

// ---------------------------------------------------------------------------
// RelationField – search & pick entries from a target collection
// ---------------------------------------------------------------------------

function entryData(entry: Entry): Record<string, unknown> {
  if (typeof entry.data === "string") {
    try {
      return JSON.parse(entry.data) as Record<string, unknown>;
    } catch {
      return {};
    }
  }
  return entry.data ?? {};
}

/** Resolve the field name to display for a relation target (presentable → first text → undefined). */
function resolvePresentable(definition?: string): string | undefined {
  if (!definition) return undefined;
  try {
    const def = JSON.parse(definition) as { fields?: ContentField[] };
    const fields = def.fields ?? [];
    const presentable = fields.find((f) => f.presentable);
    if (presentable) return presentable.name;
    const firstText = fields.find(
      (f) => f.type === "text" || f.type === "textarea",
    );
    return firstText?.name;
  } catch {
    return undefined;
  }
}

const RelationField = memo(function RelationField({
  value,
  onChange,
  siteId,
  targetSlug,
  multiple,
  maxSelect,
  readOnly,
}: {
  value: unknown;
  onChange: (val: unknown) => void;
  siteId?: string;
  targetSlug?: string;
  multiple?: boolean;
  maxSelect?: number;
  readOnly?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");

  const selectedIds: string[] = multiple
    ? Array.isArray(value)
      ? value.filter((v): v is string => typeof v === "string")
      : []
    : typeof value === "string" && value
      ? [value]
      : [];

  const enabled = !!siteId && !!targetSlug && (open || selectedIds.length > 0);

  const { data: targetCol } = useQuery({
    queryKey: ["collection", siteId, targetSlug],
    queryFn: () => getCollection(siteId as string, targetSlug as string),
    enabled: !!siteId && !!targetSlug,
  });

  const { data: entriesRes } = useQuery({
    queryKey: ["relation-entries", siteId, targetSlug, search],
    queryFn: () =>
      getEntries(siteId as string, {
        type: targetSlug,
        search: search || undefined,
        pageSize: 20,
      }),
    enabled,
  });

  const presentable = useMemo(
    () => resolvePresentable(targetCol?.definition),
    [targetCol],
  );

  const items = entriesRes?.items ?? [];

  const labelFor = useCallback(
    (entry: Entry): string => {
      const data = entryData(entry);
      const fromField = presentable ? data[presentable] : undefined;
      if (typeof fromField === "string" && fromField) return fromField;
      return entry.slug || entry.id;
    },
    [presentable],
  );

  const labelMap = useMemo(() => {
    const map: Record<string, string> = {};
    for (const e of items) map[e.id] = labelFor(e);
    return map;
  }, [items, labelFor]);

  const toggle = (id: string) => {
    if (readOnly) return;
    if (multiple) {
      if (selectedIds.includes(id)) {
        onChange(selectedIds.filter((s) => s !== id));
      } else {
        if (maxSelect && selectedIds.length >= maxSelect) return;
        onChange([...selectedIds, id]);
      }
    } else {
      onChange(selectedIds.includes(id) ? "" : id);
      setOpen(false);
    }
  };

  if (!targetSlug) {
    return (
      <p className="text-xs text-muted-foreground">
        No target collection configured for this relation.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {selectedIds.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {selectedIds.map((id) => (
            <span
              key={id}
              className="inline-flex max-w-full items-center gap-1 rounded-md border bg-background px-2 py-1 text-xs"
            >
              <span className="truncate">
                {labelMap[id] ?? `${id.slice(0, 8)}…`}
              </span>
              {!readOnly && (
                <button
                  type="button"
                  className="text-muted-foreground hover:text-foreground"
                  onClick={() => toggle(id)}
                  aria-label="Remove"
                >
                  <X className="size-3" />
                </button>
              )}
            </span>
          ))}
        </div>
      )}
      {!readOnly && (
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger
            render={
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="w-fit"
              />
            }
          >
            <Plus data-icon="inline-start" />
            {multiple ? "Add reference" : "Select reference"}
          </PopoverTrigger>
          <PopoverContent
            align="start"
            className="w-[min(22rem,calc(100vw-2rem))] p-0"
          >
            <Command shouldFilter={false}>
              <CommandInput
                value={search}
                onValueChange={setSearch}
                placeholder="Search…"
              />
              <CommandList>
                <CommandEmpty>No matching entries.</CommandEmpty>
                <CommandGroup>
                  {items.map((entry) => {
                    const isSel = selectedIds.includes(entry.id);
                    return (
                      <CommandItem
                        key={entry.id}
                        value={entry.id}
                        onSelect={() => toggle(entry.id)}
                      >
                        <Check
                          className={`size-4 ${isSel ? "opacity-100" : "opacity-0"}`}
                        />
                        <span className="truncate">{labelFor(entry)}</span>
                      </CommandItem>
                    );
                  })}
                </CommandGroup>
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
});
