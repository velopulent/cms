import { useState } from "react";
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
}

export function DynamicForm({
  fields,
  form,
  prefix = "data",
  siteId,
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
}: {
  field: ContentField;
  form: DynamicFormProps["form"];
  prefix: string;
  siteId?: string;
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
              onChange={f.handleChange}
              onBlur={f.handleBlur}
              isInvalid={isInvalid}
              siteId={siteId}
              fieldName={fieldName}
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
}: {
  field: ContentField;
  value: unknown;
  onChange: (val: unknown) => void;
  onBlur: () => void;
  isInvalid: boolean;
  siteId?: string;
  fieldName: string;
}) {
  const strValue = typeof value === "string" ? value : "";
  const numValue = typeof value === "number" ? value : 0;
  const boolValue = typeof value === "boolean" ? value : false;

  switch (field.type) {
    case "text":
      return (
        <Input
          id={fieldName}
          value={strValue}
          onBlur={onBlur}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={isInvalid}
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
          id={fieldName}
          type="number"
          value={numValue}
          onBlur={onBlur}
          onChange={(e) => onChange(Number(e.target.value) || 0)}
          aria-invalid={isInvalid}
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
        />
      );

    case "select":
      return (
        <Select
          value={strValue}
          onValueChange={(val) => onChange(val as string)}
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
            id={fieldName}
            placeholder="Image URL"
            value={strValue}
            onBlur={onBlur}
            onChange={(e) => onChange(e.target.value)}
            aria-invalid={isInvalid}
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
      return (
        <FileField
          value={strValue}
          onChange={onChange}
          siteId={siteId}
          isInvalid={isInvalid}
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
        />
      );
  }
}

function FileField({
  value,
  onChange,
  siteId,
  isInvalid,
}: {
  value: string;
  onChange: (val: unknown) => void;
  siteId?: string;
  isInvalid: boolean;
}) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [selectedFileInfo, setSelectedFileInfo] = useState<FileItem | null>(
    null,
  );

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
        aria-invalid={isInvalid}
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
