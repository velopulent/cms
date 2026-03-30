import { useState } from "react";
import { FilePickerDialog } from "@/components/file-picker-dialog";
import { TiptapEditor } from "@/components/tiptap-editor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  values: Record<string, unknown>;
  onChange: (values: Record<string, unknown>) => void;
  siteId?: string;
}

export function DynamicForm({
  fields,
  values,
  onChange,
  siteId,
}: DynamicFormProps) {
  const updateField = (name: string, value: unknown) => {
    onChange({ ...values, [name]: value });
  };

  return (
    <div className="flex flex-col gap-4">
      {fields.map((field) => (
        <DynamicField
          key={field.name}
          field={field}
          value={values[field.name]}
          onChange={(val) => updateField(field.name, val)}
          siteId={siteId}
        />
      ))}
    </div>
  );
}

function DynamicField({
  field,
  value,
  onChange,
  siteId,
}: {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
  siteId?: string;
}) {
  const label = field.name
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());

  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm font-medium">
        {label}
        {field.required && <span className="ml-1 text-destructive">*</span>}
      </p>
      <FieldInput
        field={field}
        value={value}
        onChange={onChange}
        siteId={siteId}
      />
    </div>
  );
}

function FieldInput({
  field,
  value,
  onChange,
  siteId,
}: {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
  siteId?: string;
}) {
  const strValue = typeof value === "string" ? value : "";
  const numValue = typeof value === "number" ? value : 0;
  const boolValue = typeof value === "boolean" ? value : false;

  switch (field.type) {
    case "text":
      return (
        <Input
          placeholder={field.name}
          value={strValue}
          onChange={(e) => onChange(e.target.value)}
        />
      );

    case "textarea":
      return (
        <Textarea
          placeholder={field.name}
          value={strValue}
          onChange={(e) => onChange(e.target.value)}
          rows={4}
        />
      );

    case "rich_text":
      return (
        <TiptapEditor
          content={strValue}
          onChange={(html) => onChange(html)}
          placeholder={`Write ${field.name}...`}
          siteId={siteId}
        />
      );

    case "number":
      return (
        <Input
          type="number"
          value={numValue}
          onChange={(e) => onChange(Number(e.target.value) || 0)}
        />
      );

    case "boolean":
      return (
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={boolValue}
            onChange={(e) => onChange(e.target.checked)}
          />
          Enabled
        </label>
      );

    case "date":
      return (
        <Input
          type="date"
          value={strValue}
          onChange={(e) => onChange(e.target.value)}
        />
      );

    case "select":
      return (
        <Select
          items={[
            { label: `Select ${field.name}`, value: null },
            ...(field.options ?? []).map((opt) => ({
              label: opt,
              value: opt,
            })),
          ]}
          value={strValue}
          onValueChange={(val) => onChange(val as string)}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {(field.options ?? []).map((opt) => (
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
            placeholder="Image URL"
            value={strValue}
            onChange={(e) => onChange(e.target.value)}
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

    case "media":
      return <FileField value={strValue} onChange={onChange} siteId={siteId} />;

    default:
      return (
        <Input value={strValue} onChange={(e) => onChange(e.target.value)} />
      );
  }
}

function FileField({
  value,
  onChange,
  siteId,
}: {
  value: string;
  onChange: (val: unknown) => void;
  siteId?: string;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [selectedFileInfo, setSelectedFileInfo] = useState<FileItem | null>(
    null,
  );

  // value format: "/api/files/<id>" or a full external URL or empty
  const fileIdMatch = value.match(/\/api\/files\/([^/]+)/);
  const fileId = fileIdMatch ? fileIdMatch[1] : null;
  const isExternalUrl = !fileId && value.startsWith("http");
  const isVideo = selectedFileInfo?.mime_type?.startsWith("video/");

  return (
    <div className="flex flex-col gap-2">
      {value && (
        <div className="relative flex flex-col gap-3 rounded-lg border p-2">
          <div className="flex items-center gap-3">
            {isExternalUrl && (
              <img
                src={value}
                alt="Selected file"
                className="h-16 w-16 rounded object-cover"
              />
            )}
            {fileId && isVideo && selectedFileInfo?.thumbnail_url && (
              <img
                src={selectedFileInfo.thumbnail_url}
                alt="Selected file"
                className="h-16 w-16 rounded object-cover"
              />
            )}
            {fileId && !isVideo && (
              <img
                src={`/api/files/${fileId}/thumbnail`}
                alt="Selected file"
                className="h-16 w-16 rounded object-cover"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = "none";
                }}
              />
            )}
            <div className="flex-1">
              <Badge variant="secondary" className="text-xs">
                {fileId ? `File: ${fileId.slice(0, 8)}...` : "File selected"}
              </Badge>
              {isVideo && selectedFileInfo?.original_name && (
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {selectedFileInfo.original_name}
                </p>
              )}
            </div>
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
          </div>
          {isVideo && value && (
            <VideoPlayer
              src={value}
              poster={selectedFileInfo?.thumbnail_url || undefined}
              className="w-full overflow-hidden rounded"
            />
          )}
        </div>
      )}
      <Button
        type="button"
        variant="outline"
        onClick={() => setPickerOpen(true)}
        disabled={!siteId}
      >
        {value ? "Change File" : "Select File"}
      </Button>
      {siteId && (
        <FilePickerDialog
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          onSelect={(file) => {
            onChange(file.url);
            setSelectedFileInfo(file);
          }}
          siteId={siteId}
        />
      )}
    </div>
  );
}
