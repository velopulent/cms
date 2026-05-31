import { Archive, FileText, Music } from "lucide-react";
import { useEffect, useState } from "react";
import { FilePickerDialog } from "@/components/file-picker-dialog";
import { TiptapEditor } from "@/components/tiptap-editor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { VideoPlayer } from "@/components/video-player";
import type { ContentField, FileItem } from "@/lib/api";

interface DynamicFormProps {
  fields: ContentField[];
  // biome-ignore lint/suspicious/noExplicitAny: TanStack Form instance with complex generics
  form: any;
  prefix?: string;
  siteId?: string;
  readOnly?: boolean;
}

export function DynamicForm({
  fields,
  form,
  prefix = "data",
  siteId,
  readOnly,
}: DynamicFormProps) {
  return (
    <FieldGroup>
      {fields.map((field) => (
        <DynamicField
          key={field.name}
          field={field}
          form={form}
          prefix={prefix}
          siteId={siteId}
          readOnly={readOnly}
        />
      ))}
    </FieldGroup>
  );
}

function DynamicField({
  field,
  form,
  prefix,
  siteId,
  readOnly,
}: {
  field: ContentField;
  form: DynamicFormProps["form"];
  prefix: string;
  siteId?: string;
  readOnly?: boolean;
}) {
  const fieldName = `${prefix}.${field.name}`;
  const label = field.name
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());

  return (
    <form.Field
      name={fieldName}
      children={(f: any) => {
        const isInvalid = f.state.meta.isTouched && !f.state.meta.isValid;
        return (
          <Field data-invalid={isInvalid}>
            <FieldLabel htmlFor={fieldName}>
              {label}
              {field.required && (
                <span className="ml-1 text-destructive">*</span>
              )}
            </FieldLabel>
            <FieldInput
              field={field}
              value={f.state.value}
              onChange={readOnly ? () => {} : f.handleChange}
              onBlur={readOnly ? () => {} : f.handleBlur}
              isInvalid={isInvalid}
              siteId={siteId}
              fieldName={fieldName}
              readOnly={readOnly}
            />
            {isInvalid && <FieldError errors={f.state.meta.errors} />}
          </Field>
        );
      }}
    />
  );
}

function FieldInput({
  field,
  value,
  onChange,
  onBlur,
  isInvalid,
  siteId,
  fieldName,
  readOnly,
}: {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
  onBlur: () => void;
  isInvalid: boolean;
  siteId?: string;
  fieldName: string;
  readOnly?: boolean;
}) {
  const strValue = typeof value === "string" ? value : "";
  const numValue = typeof value === "number" ? value : 0;
  const boolValue = typeof value === "boolean" ? value : false;

  const normalizedAccept: string[] | undefined = Array.isArray(field.accept)
    ? field.accept
    : typeof field.accept === "string"
      ? [field.accept]
      : undefined;
  const normalizedOptions: string[] | undefined = Array.isArray(field.options)
    ? field.options
    : typeof field.options === "string"
      ? [field.options]
      : undefined;

  switch (field.type) {
    case "text":
      return (
        <Input
          id={fieldName}
          value={strValue}
          onBlur={onBlur}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={isInvalid}
          readOnly={readOnly}
          disabled={readOnly}
        />
      );

    case "textarea":
      return (
        <Textarea
          id={fieldName}
          value={strValue}
          onBlur={onBlur}
          onChange={(e) => onChange(e.target.value)}
          rows={4}
          aria-invalid={isInvalid}
          readOnly={readOnly}
          disabled={readOnly}
        />
      );

    case "rich_text":
      return (
        <TiptapEditor
          content={strValue}
          onChange={(html) => onChange(html)}
          placeholder={`Write ${field.name}...`}
          siteId={siteId}
          editable={!readOnly}
        />
      );

    case "number":
      return (
        <Input
          id={fieldName}
          type="number"
          value={numValue}
          onBlur={onBlur}
          onChange={(e) => onChange(Number(e.target.value) || 0)}
          aria-invalid={isInvalid}
          readOnly={readOnly}
          disabled={readOnly}
        />
      );

    case "boolean":
      return (
        <Field orientation="horizontal">
          <Checkbox
            id={fieldName}
            checked={boolValue}
            onCheckedChange={(checked) => onChange(!!checked)}
            aria-invalid={isInvalid}
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
          id={fieldName}
          type="date"
          value={strValue}
          onBlur={onBlur}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={isInvalid}
          readOnly={readOnly}
          disabled={readOnly}
        />
      );

    case "select":
      return (
        <Select
          value={strValue}
          onValueChange={(val) => onChange(val as string)}
          disabled={readOnly}
        >
          <SelectTrigger
            id={fieldName}
            aria-invalid={isInvalid}
            className="w-full"
          >
            <SelectValue placeholder={`Select ${field.name}`} />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {(normalizedOptions ?? []).map((opt) => (
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
            id={fieldName}
            placeholder="Image URL"
            value={strValue}
            onBlur={onBlur}
            onChange={(e) => onChange(e.target.value)}
            aria-invalid={isInvalid}
            readOnly={readOnly}
            disabled={readOnly}
          />
          {strValue && (
            <img
              src={strValue}
              alt="Preview"
              className="h-32 w-auto rounded-lg border object-cover"
            />
          )}
        </div>
      );

    case "image":
    case "video":
    case "audio":
    case "document":
    case "archive":
      return (
        <FileField
          value={strValue}
          onChange={onChange}
          siteId={siteId}
          isInvalid={isInvalid}
          readOnly={readOnly}
          category={field.type}
          accept={normalizedAccept}
        />
      );

    default:
      return (
        <Input
          id={fieldName}
          value={strValue}
          onBlur={onBlur}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={isInvalid}
          readOnly={readOnly}
          disabled={readOnly}
        />
      );
  }
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function FileField({
  value,
  onChange,
  siteId,
  isInvalid,
  readOnly,
  category,
  accept,
}: {
  value: string;
  onChange: (val: unknown) => void;
  siteId?: string;
  isInvalid: boolean;
  readOnly?: boolean;
  category?: string;
  accept?: string[];
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [selectedFileInfo, setSelectedFileInfo] = useState<FileItem | null>(
    null,
  );

  const fileIdMatch = value.match(/\/api\/files\/([^/]+)/);
  const fileId = fileIdMatch ? fileIdMatch[1] : null;
  const isExternalUrl = !fileId && value.startsWith("http");
  const isVideo = selectedFileInfo?.mime_type?.startsWith("video/");
  const isAudio = selectedFileInfo?.mime_type?.startsWith("audio/");
  const isImage =
    selectedFileInfo?.mime_type?.startsWith("image/") ||
    (!selectedFileInfo && isExternalUrl);

  useEffect(() => {
    if (!fileId || !siteId) return;

    let cancelled = false;

    fetch(`/api/dashboard/sites/${siteId}/files/${fileId}`, {
      credentials: "include",
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((data: FileItem | null) => {
        if (!cancelled && data) setSelectedFileInfo(data);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [fileId, siteId]);

  const filterAccept =
    accept?.join(", ") ||
    (category === "image"
      ? "image/*"
      : category === "video"
        ? "video/*"
        : category === "audio"
          ? "audio/*"
          : category === "document"
            ? ".pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.txt,.csv,.html,.md"
            : category === "archive"
              ? ".zip,.gz,.tar,.7z,.rar"
              : undefined);

  return (
    <div className="flex flex-col gap-2">
      {value && (
        <div className="relative flex flex-col gap-3 rounded-lg border p-2">
          <div className="flex items-center gap-3">
            {isExternalUrl && isImage && (
              <img
                src={value}
                alt="Selected file"
                className="h-16 w-16 rounded object-cover"
              />
            )}
            {isExternalUrl && !isImage && (
              <div className="flex h-16 w-16 items-center justify-center rounded bg-muted">
                <FileText className="size-6 text-muted-foreground" />
              </div>
            )}
            {fileId && isVideo && selectedFileInfo?.thumbnail_url && (
              <img
                src={selectedFileInfo.thumbnail_url}
                alt="Selected file"
                className="h-16 w-16 rounded object-cover"
              />
            )}
            {fileId && !isVideo && isImage && (
              <img
                src={`/api/files/${fileId}/thumbnail`}
                alt="Selected file"
                className="h-16 w-16 rounded object-cover"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = "none";
                }}
              />
            )}
            {fileId && isAudio && (
              <div className="flex h-16 w-16 items-center justify-center rounded bg-muted">
                <Music className="size-6 text-muted-foreground" />
              </div>
            )}
            {fileId &&
              !isVideo &&
              !isImage &&
              !isAudio &&
              selectedFileInfo?.mime_type && (
                <div className="flex h-16 w-16 items-center justify-center rounded bg-muted">
                  {selectedFileInfo.mime_type.startsWith("application/pdf") ||
                  selectedFileInfo.mime_type.startsWith("application/msword") ||
                  selectedFileInfo.mime_type.startsWith("application/vnd.") ||
                  selectedFileInfo.mime_type.startsWith("text/") ? (
                    <FileText className="size-6 text-muted-foreground" />
                  ) : (
                    <Archive className="size-6 text-muted-foreground" />
                  )}
                </div>
              )}
            <div className="flex-1">
              <Badge variant="secondary" className="text-xs">
                {selectedFileInfo?.original_name
                  ? selectedFileInfo.original_name
                  : fileId
                    ? `${fileId.slice(0, 8)}...`
                    : "File selected"}
              </Badge>
              {selectedFileInfo?.mime_type && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {selectedFileInfo.mime_type}
                  {selectedFileInfo.size
                    ? ` — ${formatFileSize(selectedFileInfo.size)}`
                    : ""}
                </p>
              )}
            </div>
            {!readOnly && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => {
                  onChange("");
                  setSelectedFileInfo(null);
                }}
              >
                Remove
              </Button>
            )}
          </div>
          {isVideo && value && (
            <VideoPlayer
              src={value}
              poster={selectedFileInfo?.thumbnail_url || undefined}
              className="w-full overflow-hidden rounded"
            />
          )}
          {isAudio && value && (
            <audio
              controls
              src={value}
              className="w-full"
            />
          )}
        </div>
      )}
      {!readOnly && (
        <Button
          type="button"
          variant="outline"
          onClick={() => setPickerOpen(true)}
          disabled={!siteId}
          aria-invalid={isInvalid}
        >
          {value ? "Change File" : "Select File"}
        </Button>
      )}
      {siteId && (
        <FilePickerDialog
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          onSelect={(file) => {
            onChange(file.url);
            setSelectedFileInfo(file);
          }}
          siteId={siteId}
          accept={filterAccept}
        />
      )}
    </div>
  );
}
